#include <ibus.h>
#include <glib/gstdio.h>
#include <stdbool.h>
#include <stddef.h>
#include <string.h>
#include <stdint.h>
#include <unistd.h>

#define SUZAKU_LOOKUP_PAGE_SIZE 6
/* Desktop shortcuts can arrive with physical Mod4 or virtual Super/Hyper/Meta. */
#define SUZAKU_SYSTEM_MODIFIERS \
    (IBUS_CONTROL_MASK | IBUS_MOD4_MASK | IBUS_SUPER_MASK | IBUS_HYPER_MASK | IBUS_META_MASK)

extern bool suzaku_host_ime_activate(void);
extern void suzaku_host_ime_deactivate(void);
extern bool suzaku_host_ime_replace_marked_text_utf8(const char *text);
extern void suzaku_host_ime_clear_marked_text(void);
extern void suzaku_host_ime_move_selection(ptrdiff_t delta);
extern void suzaku_host_ime_select_candidate(size_t index);
extern bool suzaku_host_ime_commit_selected(bool force);
extern size_t suzaku_host_ime_candidate_count(void);
extern size_t suzaku_host_ime_selected_index(void);
extern char *suzaku_host_ime_candidate_label_utf8(size_t index);
extern char *suzaku_host_ime_ibus_candidate_label_utf8(size_t index);
extern char *suzaku_host_ime_completion_text_utf8(bool explicit_only);
extern void suzaku_host_ime_enable_ibus_candidates(void);
extern char *suzaku_host_ime_take_last_committed_text_utf8(void);
extern void suzaku_host_ime_free_utf8(char *text);
extern bool suzaku_host_ime_poll_prediction(void);
extern bool suzaku_host_ime_prediction_pending(void);
extern void suzaku_host_ime_set_private(bool private_input);
extern unsigned int suzaku_host_ime_language_kind(void);
extern char *suzaku_host_ime_control_utf8(const char *command);
extern char *suzaku_host_ime_companion_snapshot_utf8(
    const char *host, uint64_t context, uint64_t revision, bool focused, bool private_input);

typedef struct _SuzakuIBusEngine {
    IBusEngine parent_instance;
    GString *input;
    gchar *completion_undo;
    gboolean sensitive;
    gboolean private_input;
} SuzakuIBusEngine;

typedef struct _SuzakuIBusEngineClass {
    IBusEngineClass parent_class;
} SuzakuIBusEngineClass;

G_DEFINE_TYPE(SuzakuIBusEngine, suzaku_ibus_engine, IBUS_TYPE_ENGINE)

static GWeakRef suzaku_last_focused_engine;
static GSocketService *suzaku_ipc_service = NULL;
static gchar *suzaku_ipc_socket_path = NULL;
static guint suzaku_prediction_source = 0;
static void suzaku_ibus_schedule_prediction(void);
static void suzaku_companion_publish(void);
static gchar *suzaku_companion_host_id = NULL;
static guint64 suzaku_companion_context = 0;
static guint64 suzaku_companion_revision = 0;
static GPtrArray *suzaku_companion_subscribers = NULL;

static gboolean suzaku_ibus_engine_is_focused(IBusEngine *engine) {
    GObject *focused = g_weak_ref_get(&suzaku_last_focused_engine);
    gboolean matches = focused == G_OBJECT(engine);
    g_clear_object(&focused);
    return matches;
}

static size_t suzaku_ibus_candidate_page_start(void) {
    size_t selected = suzaku_host_ime_selected_index();
    return (selected / SUZAKU_LOOKUP_PAGE_SIZE) * SUZAKU_LOOKUP_PAGE_SIZE;
}

static void suzaku_ibus_engine_hide(IBusEngine *engine) {
    ibus_engine_hide_preedit_text(engine);
    ibus_engine_hide_lookup_table(engine);
    ibus_engine_hide_auxiliary_text(engine);
}

static void suzaku_ibus_engine_render(SuzakuIBusEngine *self) {
    suzaku_companion_publish();
    IBusEngine *engine = IBUS_ENGINE(self);
    if (self->input->len == 0) {
        suzaku_ibus_engine_hide(engine);
        return;
    }

    IBusText *preedit = ibus_text_new_from_string(self->input->str);
    ibus_text_append_attribute(preedit, IBUS_ATTR_TYPE_UNDERLINE,
                               IBUS_ATTR_UNDERLINE_SINGLE, 0, -1);
    ibus_engine_update_preedit_text(
        engine, preedit, (guint)g_utf8_strlen(self->input->str, -1), TRUE);

    size_t candidate_count = suzaku_host_ime_candidate_count();
    if (candidate_count == 0) {
        ibus_engine_hide_lookup_table(engine);
        ibus_engine_hide_auxiliary_text(engine);
        return;
    }

    size_t selected = suzaku_host_ime_selected_index();
    IBusLookupTable *table = ibus_lookup_table_new(
        SUZAKU_LOOKUP_PAGE_SIZE,
        (guint)MIN(selected, candidate_count - 1),
        TRUE,
        FALSE);
    ibus_lookup_table_set_orientation(table, IBUS_ORIENTATION_VERTICAL);
    for (size_t index = 0; index < candidate_count; index++) {
        char *label = suzaku_host_ime_ibus_candidate_label_utf8(index);
        if (label == NULL) {
            continue;
        }
        ibus_lookup_table_append_candidate(
            table, ibus_text_new_from_string(label));
        suzaku_host_ime_free_utf8(label);
    }
    const gchar *keys[] = {"¹", "²", "³", "⁴", "⁵", "⁶"};
    for (guint index = 0; index < SUZAKU_LOOKUP_PAGE_SIZE; index++) {
        ibus_lookup_table_append_label(table, ibus_text_new_from_string(keys[index]));
    }
    ibus_engine_update_lookup_table(engine, table, TRUE);
    guint language = suzaku_host_ime_language_kind();
    const gchar *mode = language == 2 ? "EN" : language == 1 ? "拼音" : "ローマ字";
    gchar *help = g_strdup_printf(
        "%s · ᵂ词 ˢ句 ᴿ原文 · 权重₀–₁₀₀ | 1–6 选词续写 · Alt+数字 输入数字 · 空格连写 · Enter/点击提交",
        mode);
    ibus_engine_update_auxiliary_text(engine, ibus_text_new_from_string(help), TRUE);
    g_free(help);
}

static void suzaku_ibus_engine_sync_input(SuzakuIBusEngine *self) {
    g_clear_pointer(&self->completion_undo, g_free);
    if (self->input->len == 0) {
        suzaku_host_ime_clear_marked_text();
    } else {
        suzaku_host_ime_replace_marked_text_utf8(self->input->str);
    }
    suzaku_ibus_engine_render(self);
    suzaku_ibus_schedule_prediction();
}

static void suzaku_ibus_engine_append_character(SuzakuIBusEngine *self, gunichar character) {
    gchar utf8[7] = {0};
    gint length = g_unichar_to_utf8(character, utf8);
    g_string_append_len(self->input, utf8, length);
    suzaku_ibus_engine_sync_input(self);
}

static void suzaku_ibus_engine_clear_local(SuzakuIBusEngine *self) {
    g_string_truncate(self->input, 0);
    g_clear_pointer(&self->completion_undo, g_free);
}

static void suzaku_ibus_engine_clear(SuzakuIBusEngine *self) {
    suzaku_ibus_engine_clear_local(self);
    suzaku_host_ime_clear_marked_text();
    suzaku_ibus_engine_render(self);
    suzaku_ibus_schedule_prediction();
}

/* Keep the replacement in preedit; no commit event or committed context is created.
 * Backspace immediately after a replacement restores the exact previous spelling. */
static gboolean suzaku_ibus_engine_complete(SuzakuIBusEngine *self, gboolean append_space) {
    if (!suzaku_ibus_engine_is_focused(IBUS_ENGINE(self)) ||
        self->sensitive || self->input->len == 0) { return FALSE; }
    char *candidate = suzaku_host_ime_completion_text_utf8(append_space);
    if (!append_space && candidate == NULL) { return FALSE; }
    const gchar *text = candidate != NULL ? candidate : self->input->str;
    gboolean changed = g_strcmp0(text, self->input->str) != 0;
    if (!changed && !append_space) {
        suzaku_host_ime_free_utf8(candidate);
        return TRUE;
    }
    gchar *previous = changed ? g_strdup(self->input->str) : NULL;
    gchar *replacement = append_space ? g_strconcat(text, " ", NULL) : g_strdup(text);
    g_string_assign(self->input, replacement);
    g_free(replacement);
    suzaku_host_ime_free_utf8(candidate);
    suzaku_ibus_engine_sync_input(self);
    self->completion_undo = previous;
    return TRUE;
}

static gboolean suzaku_ibus_prediction_tick(gpointer data) {
    (void)data;
    if (suzaku_host_ime_poll_prediction()) {
        GObject *focused = g_weak_ref_get(&suzaku_last_focused_engine);
        if (focused != NULL) {
            suzaku_ibus_engine_render((SuzakuIBusEngine *)focused);
            g_object_unref(focused);
        }
    }
    if (!suzaku_host_ime_prediction_pending()) {
        suzaku_prediction_source = 0;
        return G_SOURCE_REMOVE;
    }
    return G_SOURCE_CONTINUE;
}

static void suzaku_ibus_schedule_prediction(void) {
    if (suzaku_host_ime_prediction_pending()) {
        if (suzaku_prediction_source == 0) {
            suzaku_prediction_source = g_timeout_add(30, suzaku_ibus_prediction_tick, NULL);
        }
    } else if (suzaku_prediction_source != 0) {
        g_source_remove(suzaku_prediction_source);
        suzaku_prediction_source = 0;
    }
}

static gboolean suzaku_ibus_engine_commit(SuzakuIBusEngine *self) {
    if (!suzaku_ibus_engine_is_focused(IBUS_ENGINE(self)) ||
        self->sensitive || self->input->len == 0 || !suzaku_host_ime_commit_selected(true)) {
        return FALSE;
    }

    char *committed = suzaku_host_ime_take_last_committed_text_utf8();
    if (committed == NULL || committed[0] == '\0') {
        suzaku_host_ime_free_utf8(committed);
        return FALSE;
    }

    ibus_engine_commit_text(IBUS_ENGINE(self), ibus_text_new_from_string(committed));
    suzaku_host_ime_free_utf8(committed);
    suzaku_ibus_engine_clear(self);
    return TRUE;
}

static void suzaku_ibus_engine_move_selection(
    SuzakuIBusEngine *self, ptrdiff_t delta) {
    if (!suzaku_ibus_engine_is_focused(IBUS_ENGINE(self)) ||
        self->sensitive || self->input->len == 0) {
        return;
    }
    g_clear_pointer(&self->completion_undo, g_free);
    suzaku_host_ime_move_selection(delta);
    suzaku_ibus_engine_render(self);
}

static gboolean suzaku_ibus_engine_process_key_event(
    IBusEngine *engine, guint keyval, guint keycode, guint state) {
    (void)keycode;
    SuzakuIBusEngine *self = (SuzakuIBusEngine *)engine;

    if (!suzaku_ibus_engine_is_focused(engine) || self->sensitive) { return FALSE; }

    if ((state & (IBUS_RELEASE_MASK | SUZAKU_SYSTEM_MODIFIERS)) != 0) {
        return FALSE;
    }
    /* Alt+digits enter literal numbers. Leave Shift's layout-resolved symbols
     * intact, and never interpret AltGr or desktop shortcuts as number choices. */
    if ((state & IBUS_MOD1_MASK) != 0 &&
        (state & (IBUS_SHIFT_MASK | IBUS_MOD5_MASK)) == 0 &&
        keyval >= IBUS_KEY_0 && keyval <= IBUS_KEY_9) {
        suzaku_ibus_engine_append_character(self, '0' + keyval - IBUS_KEY_0);
        return TRUE;
    }
    if ((state & IBUS_MOD1_MASK) != 0) {
        return FALSE;
    }
    if ((state & IBUS_MOD5_MASK) != 0 &&
        (keyval == IBUS_KEY_space || keyval == IBUS_KEY_Return || keyval == IBUS_KEY_KP_Enter)) {
        return FALSE;
    }

    if (keyval == IBUS_KEY_BackSpace && self->input->len > 0) {
        if (self->completion_undo != NULL) {
            g_string_assign(self->input, self->completion_undo);
            suzaku_ibus_engine_sync_input(self);
            return TRUE;
        }
        gchar *last = g_utf8_find_prev_char(
            self->input->str, self->input->str + self->input->len);
        g_string_truncate(self->input,
                          last == NULL ? 0 : (gsize)(last - self->input->str));
        suzaku_ibus_engine_sync_input(self);
        return TRUE;
    }
    if (keyval == IBUS_KEY_Escape && self->input->len > 0) {
        suzaku_ibus_engine_clear(self);
        return TRUE;
    }
    if (keyval == IBUS_KEY_Up || keyval == IBUS_KEY_Left ||
        keyval == IBUS_KEY_Page_Up) {
        if (self->input->len == 0) {
            return FALSE;
        }
        suzaku_ibus_engine_move_selection(
            self, keyval == IBUS_KEY_Page_Up ? -SUZAKU_LOOKUP_PAGE_SIZE : -1);
        return TRUE;
    }
    if (keyval == IBUS_KEY_ISO_Left_Tab ||
        (keyval == IBUS_KEY_Tab && (state & IBUS_SHIFT_MASK) != 0)) {
        if (self->input->len == 0) {
            return FALSE;
        }
        suzaku_ibus_engine_move_selection(self, -1);
        return TRUE;
    }
    if (keyval == IBUS_KEY_Down || keyval == IBUS_KEY_Right ||
        keyval == IBUS_KEY_Page_Down || keyval == IBUS_KEY_Tab) {
        if (self->input->len == 0) {
            return FALSE;
        }
        suzaku_ibus_engine_move_selection(
            self, keyval == IBUS_KEY_Page_Down ? SUZAKU_LOOKUP_PAGE_SIZE : 1);
        return TRUE;
    }
    /* Space is a separator within one writing stream, not a commit boundary.
     * Only an explicitly selected candidate may replace the user's spelling. */
    if (keyval == IBUS_KEY_space && self->input->len > 0) {
        return suzaku_ibus_engine_complete(self, TRUE);
    }
    if ((keyval == IBUS_KEY_Return || keyval == IBUS_KEY_KP_Enter) &&
        (state & IBUS_SHIFT_MASK) != 0 && self->input->len > 0) {
        return suzaku_ibus_engine_complete(self, FALSE);
    }
    if (keyval == IBUS_KEY_Return || keyval == IBUS_KEY_KP_Enter) {
        return suzaku_ibus_engine_commit(self);
    }

    if (self->input->len > 0 && suzaku_host_ime_candidate_count() > 0 &&
        (state & (IBUS_SHIFT_MASK | IBUS_MOD5_MASK)) == 0 &&
        keyval >= IBUS_KEY_0 && keyval <= IBUS_KEY_9) {
        if (keyval >= IBUS_KEY_1 && keyval < IBUS_KEY_1 + SUZAKU_LOOKUP_PAGE_SIZE) {
            size_t index = suzaku_ibus_candidate_page_start() +
                           (size_t)(keyval - IBUS_KEY_1);
            if (index < suzaku_host_ime_candidate_count()) {
                g_clear_pointer(&self->completion_undo, g_free);
                suzaku_host_ime_select_candidate(index);
                return suzaku_ibus_engine_complete(self, FALSE);
            }
        }
        /* An empty numbered slot must not silently insert a digit into the draft. */
        return TRUE;
    }

    guint language = suzaku_host_ime_language_kind();
    gunichar character = ibus_keyval_to_unicode(keyval);
    if ((character >= 'a' && character <= 'z') ||
        (character >= 'A' && character <= 'Z') || character == '\'' ||
        (character >= '0' && character <= '9') ||
        (language == 1 && (character == 0xfc || character == 0xdc || character == ':')) ||
        (language == 3 && character == '-') ||
        (self->input->len > 0 && character != 0 && g_unichar_isprint(character))) {
        /* Language converters normalize case themselves; retain the literal fallback here. */
        suzaku_ibus_engine_append_character(self, character);
        return TRUE;
    }

    /* Non-text keys and punctuation outside a draft belong to the application. */
    return FALSE;
}

static void suzaku_ibus_engine_focus_in(IBusEngine *engine) {
    SuzakuIBusEngine *self = (SuzakuIBusEngine *)engine;
    GObject *previous = g_weak_ref_get(&suzaku_last_focused_engine);
    if (previous != NULL && previous != G_OBJECT(engine)) {
        suzaku_ibus_engine_clear_local((SuzakuIBusEngine *)previous);
        suzaku_ibus_engine_hide(IBUS_ENGINE(previous));
    }
    g_clear_object(&previous);
    /* FocusOut can be delayed or absent. Start a clean field session here too,
     * including when IBus reuses the same engine for another input context. */
    suzaku_ibus_engine_clear_local(self);
    suzaku_host_ime_deactivate();
    g_weak_ref_set(&suzaku_last_focused_engine, G_OBJECT(engine));
    suzaku_companion_context++;
    suzaku_host_ime_set_private(self->private_input);
    suzaku_host_ime_activate();
    suzaku_ibus_engine_sync_input(self);
}

static gboolean suzaku_ibus_clear_focused_engine_if(IBusEngine *engine) {
    GObject *focused = g_weak_ref_get(&suzaku_last_focused_engine);
    gboolean matches = focused == G_OBJECT(engine);
    if (matches) {
        g_weak_ref_set(&suzaku_last_focused_engine, NULL);
        suzaku_companion_context++;
    }
    g_clear_object(&focused);
    return matches;
}

static void suzaku_ibus_engine_focus_out(IBusEngine *engine) {
    SuzakuIBusEngine *self = (SuzakuIBusEngine *)engine;
    if (!suzaku_ibus_clear_focused_engine_if(engine)) {
        suzaku_ibus_engine_clear_local(self);
        suzaku_ibus_engine_hide(engine);
        return;
    }
    suzaku_ibus_engine_clear(self);
    suzaku_host_ime_deactivate();
}

static void suzaku_ibus_engine_reset(IBusEngine *engine) {
    if (suzaku_ibus_engine_is_focused(engine)) {
        suzaku_ibus_engine_clear((SuzakuIBusEngine *)engine);
    } else {
        suzaku_ibus_engine_clear_local((SuzakuIBusEngine *)engine);
    }
}

static void suzaku_ibus_engine_set_content_type(IBusEngine *engine, guint purpose, guint hints) {
    SuzakuIBusEngine *self = (SuzakuIBusEngine *)engine;
    gboolean was_private = self->private_input;
    self->sensitive = purpose == IBUS_INPUT_PURPOSE_PASSWORD || purpose == IBUS_INPUT_PURPOSE_PIN;
    /* PRIVATE was added in IBus 1.5.26; use its ABI bit for older headers as well. */
    self->private_input = self->sensitive || (hints & (1u << 11)) != 0;
    if (!suzaku_ibus_engine_is_focused(engine)) {
        if (self->sensitive || was_private != self->private_input) { suzaku_ibus_engine_clear_local(self); }
        return;
    }
    suzaku_host_ime_set_private(self->private_input);
    /* A privacy transition must not carry an old private preedit into a later model request. */
    if (self->sensitive || was_private != self->private_input) {
        suzaku_ibus_engine_clear(self);
    }
    suzaku_ibus_schedule_prediction();
}

static void suzaku_ibus_engine_enable(IBusEngine *engine) {
    if (suzaku_ibus_engine_is_focused(engine)) {
        suzaku_host_ime_activate();
    }
}

static void suzaku_ibus_engine_disable(IBusEngine *engine) {
    if (!suzaku_ibus_clear_focused_engine_if(engine)) {
        suzaku_ibus_engine_clear_local((SuzakuIBusEngine *)engine);
        return;
    }
    suzaku_ibus_engine_clear((SuzakuIBusEngine *)engine);
    suzaku_host_ime_deactivate();
}

static void suzaku_ibus_engine_cursor_up(IBusEngine *engine) {
    suzaku_ibus_engine_move_selection((SuzakuIBusEngine *)engine, -1);
}

static void suzaku_ibus_engine_cursor_down(IBusEngine *engine) {
    suzaku_ibus_engine_move_selection((SuzakuIBusEngine *)engine, 1);
}

static void suzaku_ibus_engine_page_up(IBusEngine *engine) {
    suzaku_ibus_engine_move_selection(
        (SuzakuIBusEngine *)engine, -SUZAKU_LOOKUP_PAGE_SIZE);
}

static void suzaku_ibus_engine_page_down(IBusEngine *engine) {
    suzaku_ibus_engine_move_selection(
        (SuzakuIBusEngine *)engine, SUZAKU_LOOKUP_PAGE_SIZE);
}

static void suzaku_ibus_engine_candidate_clicked(
    IBusEngine *engine, guint index, guint button, guint state) {
    SuzakuIBusEngine *self = (SuzakuIBusEngine *)engine;
    if (!suzaku_ibus_engine_is_focused(engine) || self->sensitive || button != 1 ||
        (state & (SUZAKU_SYSTEM_MODIFIERS | IBUS_MOD1_MASK | IBUS_RELEASE_MASK)) != 0) {
        return;
    }
    size_t candidate_count = suzaku_host_ime_candidate_count();
    size_t absolute_index = index;
    if (candidate_count > SUZAKU_LOOKUP_PAGE_SIZE &&
        index < SUZAKU_LOOKUP_PAGE_SIZE) {
        absolute_index = suzaku_ibus_candidate_page_start() + index;
    }
    if (self->input->len == 0 || absolute_index >= candidate_count) {
        return;
    }
    suzaku_host_ime_select_candidate(absolute_index);
    suzaku_ibus_engine_commit(self);
}

static void suzaku_ibus_engine_finalize(GObject *object) {
    SuzakuIBusEngine *self = (SuzakuIBusEngine *)object;
    g_clear_pointer(&self->completion_undo, g_free);
    if (self->input != NULL) {
        g_string_free(self->input, TRUE);
        self->input = NULL;
    }
    G_OBJECT_CLASS(suzaku_ibus_engine_parent_class)->finalize(object);
}

static void suzaku_ibus_engine_class_init(SuzakuIBusEngineClass *class) {
    GObjectClass *object_class = G_OBJECT_CLASS(class);
    IBusEngineClass *engine_class = IBUS_ENGINE_CLASS(class);
    object_class->finalize = suzaku_ibus_engine_finalize;
    engine_class->process_key_event = suzaku_ibus_engine_process_key_event;
    engine_class->focus_in = suzaku_ibus_engine_focus_in;
    engine_class->focus_out = suzaku_ibus_engine_focus_out;
    engine_class->reset = suzaku_ibus_engine_reset;
    engine_class->enable = suzaku_ibus_engine_enable;
    engine_class->disable = suzaku_ibus_engine_disable;
    engine_class->cursor_up = suzaku_ibus_engine_cursor_up;
    engine_class->cursor_down = suzaku_ibus_engine_cursor_down;
    engine_class->page_up = suzaku_ibus_engine_page_up;
    engine_class->page_down = suzaku_ibus_engine_page_down;
    engine_class->candidate_clicked = suzaku_ibus_engine_candidate_clicked;
    engine_class->set_content_type = suzaku_ibus_engine_set_content_type;
}

static void suzaku_ibus_engine_init(SuzakuIBusEngine *self) {
    suzaku_host_ime_enable_ibus_candidates();
    self->input = g_string_new(NULL);
}

static void suzaku_ibus_bus_disconnected(IBusBus *bus, gpointer user_data) {
    (void)bus;
    (void)user_data;
    ibus_quit();
}

#include "ibus_companion.inc.c"
#include "ibus_ipc.inc.c"

static void suzaku_ibus_ipc_dispatch(SuzakuIpcClient *client) {
    GSocketConnection *connection = client->connection;
    gsize bytes_read = client->input->len;
    g_byte_array_append(client->input, (const guint8 *)"", 1);
    const gchar *request = (const gchar *)client->input->data;
    if (bytes_read == 1 && request[0] == 'W') {
        suzaku_companion_subscribe(connection);
        // The subscriber list owns the stream now; discard only request state.
        suzaku_ipc_finish(client, FALSE);
        return;
    }

    if (bytes_read > 0 && bytes_read < 128 &&
        (request[0] == 'S' || request[0] == 'L' || request[0] == 'P' || request[0] == 'R' || request[0] == 'U') &&
        g_utf8_validate(request, (gssize)bytes_read, NULL)) {
        unsigned int previous_language = suzaku_host_ime_language_kind();
        char *response = suzaku_host_ime_control_utf8(request);
        if (response != NULL) {
            if (strstr(response, "\"ok\":true") != NULL) {
                GObject *focused = g_weak_ref_get(&suzaku_last_focused_engine);
                if (focused != NULL) {
                    SuzakuIBusEngine *engine = (SuzakuIBusEngine *)focused;
                    /* Reloading a provider keeps the current field's draft. A language
                     * boundary still clears both the Rust and IBus compositions. */
                    if (request[0] == 'L' || (request[0] == 'R' &&
                        previous_language != suzaku_host_ime_language_kind())) {
                        suzaku_ibus_engine_clear(engine);
                    }
                    if (request[0] != 'S') { suzaku_ibus_engine_render(engine); }
                    g_object_unref(focused);
                }
                suzaku_ibus_schedule_prediction();
            }
            suzaku_ipc_reply(client, response);
            suzaku_host_ime_free_utf8(response);
        } else {
            suzaku_ipc_reply(client, "0");
        }
        return;
    }

    gboolean delivered = FALSE;
    if (bytes_read > 1 && request[0] == 'A' &&
        g_utf8_validate(request, (gssize)bytes_read, NULL)) {
        delivered = suzaku_companion_action(request + 1);
    } else if (bytes_read > 1 && request[0] == 'C' &&
        client->context == suzaku_companion_context &&
        g_utf8_validate(request + 1, (gssize)bytes_read - 1, NULL)) {
        GObject *focused = g_weak_ref_get(&suzaku_last_focused_engine);
        if (focused != NULL) {
            SuzakuIBusEngine *engine = (SuzakuIBusEngine *)focused;
            if (!engine->sensitive) {
                ibus_engine_commit_text(
                    IBUS_ENGINE(engine), ibus_text_new_from_string(request + 1));
                delivered = TRUE;
                suzaku_ibus_engine_clear(engine);
            }
            g_object_unref(focused);
        }
    } else if (bytes_read == 1 && request[0] == 'Q') {
        delivered = TRUE;
    }

    suzaku_ipc_reply(client, delivered ? "1" : "0");
}

static gboolean suzaku_ibus_start_ipc_service(void) {
    gchar *socket_directory = g_build_filename(
        g_get_user_runtime_dir(), "suzaku-ime", NULL);
    if (g_mkdir_with_parents(socket_directory, 0700) != 0) {
        g_printerr("Suzaku IBus host could not create runtime directory %s.\n",
                   socket_directory);
        g_free(socket_directory);
        return FALSE;
    }
    g_chmod(socket_directory, 0700);
    suzaku_ipc_socket_path = g_build_filename(
        socket_directory, "host.sock", NULL);
    g_free(socket_directory);
    if (g_file_test(suzaku_ipc_socket_path, G_FILE_TEST_EXISTS)) {
        g_unlink(suzaku_ipc_socket_path);
    }

    suzaku_ipc_service = g_socket_service_new();
    GSocketAddress *address = g_unix_socket_address_new(suzaku_ipc_socket_path);
    GError *error = NULL;
    gboolean added = g_socket_listener_add_address(
        G_SOCKET_LISTENER(suzaku_ipc_service),
        address,
        G_SOCKET_TYPE_STREAM,
        G_SOCKET_PROTOCOL_DEFAULT,
        NULL,
        NULL,
        &error);
    g_object_unref(address);
    if (!added) {
        g_printerr("Suzaku IBus host could not listen on %s: %s\n",
                   suzaku_ipc_socket_path,
                   error == NULL ? "unknown error" : error->message);
        g_clear_error(&error);
        g_clear_object(&suzaku_ipc_service);
        g_clear_pointer(&suzaku_ipc_socket_path, g_free);
        return FALSE;
    }

    g_chmod(suzaku_ipc_socket_path, 0600);
    g_signal_connect(suzaku_ipc_service, "incoming",
                     G_CALLBACK(suzaku_ibus_ipc_incoming), NULL);
    g_socket_service_start(suzaku_ipc_service);
    return TRUE;
}

static void suzaku_ibus_stop_ipc_service(void) {
    suzaku_ipc_close_clients();
    g_clear_pointer(&suzaku_companion_subscribers, g_ptr_array_unref);
    g_clear_pointer(&suzaku_companion_host_id, g_free);
    if (suzaku_ipc_service != NULL) {
        g_socket_service_stop(suzaku_ipc_service);
        g_clear_object(&suzaku_ipc_service);
    }
    if (suzaku_ipc_socket_path != NULL) {
        g_unlink(suzaku_ipc_socket_path);
        g_clear_pointer(&suzaku_ipc_socket_path, g_free);
    }
}

int suzaku_linux_ibus_run(
    const char *component_name, const char *engine_name, const char *version) {
    if (component_name == NULL || engine_name == NULL || version == NULL) {
        return 2;
    }

    ibus_init();
    suzaku_companion_host_id = g_uuid_string_random();
    suzaku_companion_subscribers = g_ptr_array_new_with_free_func(g_object_unref);
    g_weak_ref_init(&suzaku_last_focused_engine, NULL);
    IBusBus *bus = ibus_bus_new();
    if (bus == NULL || !ibus_bus_is_connected(bus)) {
        g_printerr("Suzaku IBus host could not connect to the IBus daemon.\n");
        g_clear_object(&bus);
        g_weak_ref_clear(&suzaku_last_focused_engine);
        return 3;
    }

    g_signal_connect(bus, "disconnected",
                     G_CALLBACK(suzaku_ibus_bus_disconnected), NULL);
    IBusFactory *factory = ibus_factory_new(ibus_bus_get_connection(bus));
    ibus_factory_add_engine(factory, engine_name, suzaku_ibus_engine_get_type());

    guint32 reply = ibus_bus_request_name(bus, component_name, 0);
    if (reply != IBUS_BUS_REQUEST_NAME_REPLY_PRIMARY_OWNER &&
        reply != IBUS_BUS_REQUEST_NAME_REPLY_ALREADY_OWNER) {
        g_printerr("Suzaku IBus host could not own component name %s (reply %u).\n",
                   component_name, reply);
        g_object_unref(factory);
        g_object_unref(bus);
        g_weak_ref_clear(&suzaku_last_focused_engine);
        return 4;
    }

    gchar *executable = g_file_read_link("/proc/self/exe", NULL);
    const gchar *host_command = executable != NULL ? executable : "linux_ime_host";
    gchar *component_command = g_strdup_printf("%s --ibus", host_command);
    IBusComponent *component = ibus_component_new(
        component_name,
        "Suzaku adaptive input method",
        version,
        "MIT",
        "Suzaku contributors",
        "https://github.com/chiharu-kiryu/Suzaku-map",
        component_command,
        "suzaku-map");
    IBusEngineDesc *engine_description = ibus_engine_desc_new(
        engine_name,
        "Suzaku",
        "Suzaku adaptive candidate engine",
        "zh",
        "MIT",
        "Suzaku contributors",
        "input-keyboard",
        "default");
    ibus_component_add_engine(component, engine_description);
    g_object_ref_sink(component);

    if (!ibus_bus_register_component(bus, component)) {
        g_printerr("Suzaku IBus host could not register its runtime component.\n");
        g_object_unref(component);
        g_free(component_command);
        g_free(executable);
        g_object_unref(factory);
        g_object_unref(bus);
        g_weak_ref_clear(&suzaku_last_focused_engine);
        return 5;
    }
    g_object_unref(component);
    g_free(component_command);
    g_free(executable);

    if (!suzaku_ibus_start_ipc_service()) {
        g_object_unref(factory);
        g_object_unref(bus);
        g_weak_ref_clear(&suzaku_last_focused_engine);
        return 6;
    }

    g_print("Suzaku IBus host ready: component=%s engine=%s ipc=%s\n",
            component_name, engine_name, suzaku_ipc_socket_path);
    ibus_main();

    if (suzaku_prediction_source != 0) {
        g_source_remove(suzaku_prediction_source);
        suzaku_prediction_source = 0;
    }

    suzaku_ibus_stop_ipc_service();
    g_weak_ref_clear(&suzaku_last_focused_engine);
    g_object_unref(factory);
    g_object_unref(bus);
    return 0;
}

typedef struct {
    gchar *committed;
    gsize committed_capacity;
    gboolean commit_received;
    gchar *preedit;
    gsize preedit_capacity;
    gboolean preedit_received;
    gchar *primary_candidate;
    gsize primary_candidate_capacity;
    size_t candidate_count;
    size_t page_size;
    size_t selected_index;
    gboolean lookup_received;
    gboolean preedit_visible;
    gboolean lookup_visible;
    size_t ai_candidate_index;
} SuzakuIBusProbe;

static void suzaku_ibus_probe_commit_text(
    IBusInputContext *context, IBusText *text, gpointer user_data) {
    (void)context;
    SuzakuIBusProbe *probe = (SuzakuIBusProbe *)user_data;
    const gchar *value = ibus_text_get_text(text);
    if (value == NULL || value[0] == '\0') {
        return;
    }
    g_strlcpy(probe->committed, value, probe->committed_capacity);
    probe->commit_received = TRUE;
}

static void suzaku_ibus_probe_update_preedit_text(
    IBusInputContext *context,
    IBusText *text,
    guint cursor_pos,
    gboolean visible,
    gpointer user_data) {
    (void)context;
    (void)cursor_pos;
    SuzakuIBusProbe *probe = (SuzakuIBusProbe *)user_data;
    probe->preedit_visible = visible;
    if (!visible) {
        return;
    }

    const gchar *value = ibus_text_get_text(text);
    if (value == NULL || value[0] == '\0') {
        return;
    }
    g_strlcpy(probe->preedit, value, probe->preedit_capacity);
    probe->preedit_received = TRUE;
}

static void suzaku_ibus_probe_update_lookup_table(
    IBusInputContext *context,
    IBusLookupTable *table,
    gboolean visible,
    gpointer user_data) {
    (void)context;
    SuzakuIBusProbe *probe = (SuzakuIBusProbe *)user_data;
    probe->lookup_visible = visible;
    if (!visible) {
        return;
    }

    guint candidate_count = ibus_lookup_table_get_number_of_candidates(table);
    if (candidate_count == 0) {
        return;
    }

    guint selected_index = MIN(
        ibus_lookup_table_get_cursor_pos(table), candidate_count - 1);
    IBusText *candidate = ibus_lookup_table_get_candidate(table, selected_index);
    if (candidate == NULL) {
        return;
    }
    const gchar *value = ibus_text_get_text(candidate);
    if (value == NULL || value[0] == '\0') {
        return;
    }

    g_strlcpy(
        probe->primary_candidate,
        value,
        probe->primary_candidate_capacity);
    probe->candidate_count = candidate_count;
    probe->page_size = ibus_lookup_table_get_page_size(table);
    probe->selected_index = selected_index;
    probe->lookup_received = TRUE;
    for (guint index = 0; index < candidate_count; index++) {
        IBusText *entry = ibus_lookup_table_get_candidate(table, index);
        const gchar *label = entry == NULL ? NULL : ibus_text_get_text(entry);
        if (label != NULL && strstr(label, "ᴬᴵ") != NULL) {
            probe->ai_candidate_index = index;
            break;
        }
    }
}

static void suzaku_ibus_probe_hide_preedit(
    IBusInputContext *context, gpointer user_data) {
    (void)context;
    ((SuzakuIBusProbe *)user_data)->preedit_visible = FALSE;
}

static void suzaku_ibus_probe_hide_lookup(
    IBusInputContext *context, gpointer user_data) {
    (void)context;
    ((SuzakuIBusProbe *)user_data)->lookup_visible = FALSE;
}

static void suzaku_ibus_probe_pump_events(void) {
    while (g_main_context_iteration(NULL, FALSE)) {
    }
}

static gboolean suzaku_ibus_probe_wait_for_engine(
    IBusInputContext *context, const gchar *engine_name) {
    for (guint attempt = 0; attempt < 200; attempt++) {
        suzaku_ibus_probe_pump_events();
        IBusEngineDesc *description = ibus_input_context_get_engine(context);
        if (description != NULL) {
            gboolean matches = g_strcmp0(
                ibus_engine_desc_get_name(description), engine_name) == 0;
            g_object_unref(description);
            if (matches) {
                return TRUE;
            }
        }
        g_usleep(10000);
    }
    return FALSE;
}

static IBusEngineDesc *suzaku_ibus_probe_wait_for_current_engine(
    IBusInputContext *context) {
    for (guint attempt = 0; attempt < 200; attempt++) {
        suzaku_ibus_probe_pump_events();
        IBusEngineDesc *description = ibus_input_context_get_engine(context);
        if (description != NULL) {
            return description;
        }
        g_usleep(10000);
    }
    return NULL;
}

static gboolean suzaku_ibus_probe_send_ipc_commit(const gchar *text) {
    gchar *socket_path = g_build_filename(
        g_get_user_runtime_dir(), "suzaku-ime", "host.sock", NULL);
    GSocketAddress *address = g_unix_socket_address_new(socket_path);
    g_free(socket_path);
    GSocketClient *client = g_socket_client_new();
    GError *error = NULL;
    GSocketConnection *connection = g_socket_client_connect(
        client, G_SOCKET_CONNECTABLE(address), NULL, &error);
    g_object_unref(address);
    g_object_unref(client);
    if (connection == NULL) {
        g_clear_error(&error);
        return FALSE;
    }

    GOutputStream *output = g_io_stream_get_output_stream(G_IO_STREAM(connection));
    const gchar operation = 'C';
    gsize written = 0;
    gboolean sent = g_output_stream_write_all(
        output, &operation, 1, &written, NULL, &error) &&
        g_output_stream_write_all(
            output, text, strlen(text), &written, NULL, &error) &&
        g_socket_shutdown(
            g_socket_connection_get_socket(connection), FALSE, TRUE, &error);
    if (!sent) {
        g_clear_error(&error);
        g_object_unref(connection);
        return FALSE;
    }

    gchar response = '0';
    gsize read = 0;
    gboolean acknowledged = g_input_stream_read_all(
        g_io_stream_get_input_stream(G_IO_STREAM(connection)),
        &response,
        1,
        &read,
        NULL,
        &error) && read == 1 && response == '1';
    g_clear_error(&error);
    g_object_unref(connection);
    return acknowledged;
}

int suzaku_linux_ibus_probe_roundtrip(
    const char *engine_name,
    const char *seed,
    bool via_ipc,
    bool wait_for_llm,
    bool complete_word,
    unsigned int privacy_mode,
    char *committed,
    size_t committed_capacity,
    char *preedit,
    size_t preedit_capacity,
    char *primary_candidate,
    size_t primary_candidate_capacity,
    size_t *candidate_count,
    size_t *page_size,
    size_t *selected_index) {
    if (engine_name == NULL || seed == NULL || seed[0] == '\0' ||
        committed == NULL || committed_capacity == 0 ||
        preedit == NULL || preedit_capacity == 0 ||
        primary_candidate == NULL || primary_candidate_capacity == 0 ||
        candidate_count == NULL || page_size == NULL || selected_index == NULL) {
        return 1;
    }
    committed[0] = '\0';
    preedit[0] = '\0';
    primary_candidate[0] = '\0';
    *candidate_count = 0;
    *page_size = 0;
    *selected_index = 0;

    ibus_init();
    IBusBus *bus = ibus_bus_new();
    if (bus == NULL || !ibus_bus_is_connected(bus)) {
        g_clear_object(&bus);
        return 2;
    }

    IBusInputContext *context = ibus_bus_create_input_context(
        bus, "suzaku-native-roundtrip-probe");
    if (context == NULL) {
        g_object_unref(bus);
        return 3;
    }

    SuzakuIBusProbe probe = {
        .committed = committed,
        .committed_capacity = committed_capacity,
        .commit_received = FALSE,
        .preedit = preedit,
        .preedit_capacity = preedit_capacity,
        .preedit_received = FALSE,
        .primary_candidate = primary_candidate,
        .primary_candidate_capacity = primary_candidate_capacity,
        .candidate_count = 0,
        .page_size = 0,
        .selected_index = 0,
        .lookup_received = FALSE,
        .ai_candidate_index = (size_t)-1,
    };
    g_signal_connect(context, "commit-text",
                     G_CALLBACK(suzaku_ibus_probe_commit_text), &probe);
    g_signal_connect(context, "update-preedit-text",
                     G_CALLBACK(suzaku_ibus_probe_update_preedit_text), &probe);
    g_signal_connect(context, "update-lookup-table",
                     G_CALLBACK(suzaku_ibus_probe_update_lookup_table), &probe);
    g_signal_connect(context, "hide-preedit-text",
                     G_CALLBACK(suzaku_ibus_probe_hide_preedit), &probe);
    g_signal_connect(context, "hide-lookup-table",
                     G_CALLBACK(suzaku_ibus_probe_hide_lookup), &probe);
    ibus_input_context_set_capabilities(
        context, IBUS_CAP_FOCUS | IBUS_CAP_PREEDIT_TEXT | IBUS_CAP_LOOKUP_TABLE);
    ibus_input_context_focus_in(context);
    suzaku_ibus_probe_pump_events();

    int result = 0;
    gchar *completion_expected = NULL;
    gboolean engine_switched = FALSE;
    G_GNUC_BEGIN_IGNORE_DEPRECATIONS
    gboolean use_global_engine = ibus_bus_get_use_global_engine(bus);
    G_GNUC_END_IGNORE_DEPRECATIONS
    IBusEngineDesc *previous_description = use_global_engine
        ? suzaku_ibus_probe_wait_for_current_engine(context)
        : NULL;
    gchar *previous_engine = previous_description == NULL
        ? NULL
        : g_strdup(ibus_engine_desc_get_name(previous_description));
    g_clear_object(&previous_description);
    if (use_global_engine &&
        (previous_engine == NULL || g_strcmp0(previous_engine, "dummy") == 0)) {
        result = 4;
        goto cleanup;
    }

    if (use_global_engine) {
        if (!ibus_bus_set_global_engine(bus, engine_name)) {
            result = 5;
            goto cleanup;
        }
        engine_switched = TRUE;
    } else {
        ibus_input_context_set_engine(context, engine_name);
    }
    if (!suzaku_ibus_probe_wait_for_engine(context, engine_name)) {
        result = 6;
        goto cleanup;
    }

    ibus_input_context_set_content_type(context,
        privacy_mode == 2 ? IBUS_INPUT_PURPOSE_PASSWORD : IBUS_INPUT_PURPOSE_FREE_FORM,
        privacy_mode != 0 ? 1u << 11 : 0);
    suzaku_ibus_probe_pump_events();
    if (privacy_mode == 2) {
        if (ibus_input_context_process_key_event(context, IBUS_KEY_a, 0, 0)) { result = 18; }
        if (suzaku_ibus_probe_send_ipc_commit(seed)) { result = 18; }
        suzaku_ibus_probe_pump_events();
        if (probe.preedit_visible || probe.lookup_visible || probe.commit_received) { result = 18; }
        goto cleanup;
    }

    suzaku_ibus_probe_pump_events();
    if (probe.preedit_visible || probe.lookup_visible) {
        result = 15;
        goto cleanup;
    }

    if (via_ipc) {
        if (!suzaku_ibus_probe_send_ipc_commit(seed)) {
            result = 11;
            goto cleanup;
        }
    } else {
        for (const gchar *cursor = seed; *cursor != '\0';
             cursor = g_utf8_next_char(cursor)) {
            gunichar character = g_utf8_get_char(cursor);
            guint modifiers = character >= '0' && character <= '9' ? IBUS_MOD1_MASK : 0;
            if (!ibus_input_context_process_key_event(context, character, 0, modifiers)) {
                result = 7;
                goto cleanup;
            }
        }
        for (guint attempt = 0;
             attempt < 100 &&
                 (!probe.preedit_received || !probe.lookup_received);
             attempt++) {
            suzaku_ibus_probe_pump_events();
            g_usleep(10000);
        }
        if (!probe.preedit_received || !probe.preedit_visible) {
            result = 12;
            goto cleanup;
        }
        if (!probe.lookup_received || !probe.lookup_visible) {
            result = 13;
            goto cleanup;
        }
        if (probe.page_size != SUZAKU_LOOKUP_PAGE_SIZE) {
            result = 14;
            goto cleanup;
        }
        if (wait_for_llm || privacy_mode == 1) {
            guint attempts = wait_for_llm ? 600 : 160;
            for (guint attempt = 0; attempt < attempts && probe.ai_candidate_index == (size_t)-1; attempt++) {
                suzaku_ibus_probe_pump_events();
                g_usleep(10000);
            }
            if (wait_for_llm && probe.ai_candidate_index == (size_t)-1) { result = 17; goto cleanup; }
            if (privacy_mode == 1 && probe.ai_candidate_index != (size_t)-1) { result = 19; goto cleanup; }
        }
        if (complete_word) {
            if (probe.candidate_count < 2 || !ibus_input_context_process_key_event(context, IBUS_KEY_Tab, 0, 0)) {
                result = 20;
                goto cleanup;
            }
            for (guint attempt = 0; attempt < 100 && probe.selected_index != 1; attempt++) {
                suzaku_ibus_probe_pump_events();
                g_usleep(10000);
            }
            if (probe.selected_index != 1) { result = 20; goto cleanup; }
            /* Keep an independent expectation: Space refreshes the lookup table. */
            const gchar *annotation = strrchr(probe.primary_candidate, ' ');
            gchar *plain = annotation == NULL ? g_strdup(probe.primary_candidate) :
                g_strndup(probe.primary_candidate, (gsize)(annotation - probe.primary_candidate));
            completion_expected = g_strconcat(plain, " ", NULL);
            g_free(plain);
            if (!ibus_input_context_process_key_event(context, IBUS_KEY_space, 0, 0)) {
                result = 22; goto cleanup;
            }
            for (guint attempt = 0; attempt < 100 &&
                 g_strcmp0(probe.preedit, completion_expected) != 0; attempt++) {
                suzaku_ibus_probe_pump_events();
                g_usleep(10000);
            }
            if (probe.commit_received || !probe.preedit_visible ||
                g_strcmp0(probe.preedit, completion_expected) != 0) {
                result = 22; goto cleanup;
            }
        }
        /* Navigate by absolute candidate position, not page-local number keys.
         * AI results can be on a later page; number choices now keep composing. */
        if (wait_for_llm) {
            ptrdiff_t delta = (ptrdiff_t)probe.ai_candidate_index - (ptrdiff_t)probe.selected_index;
            guint select_key = delta < 0 ? IBUS_KEY_ISO_Left_Tab : IBUS_KEY_Tab;
            size_t steps = (size_t)(delta < 0 ? -delta : delta);
            for (size_t step = 0; step < steps; step++) {
                if (!ibus_input_context_process_key_event(context, select_key, 0, 0)) {
                    result = 8;
                    goto cleanup;
                }
            }
        }
        if (!ibus_input_context_process_key_event(context, IBUS_KEY_Return, 0, 0)) {
            result = 8;
            goto cleanup;
        }
    }

    for (guint attempt = 0;
         attempt < 100 && (!probe.commit_received ||
             probe.preedit_visible || probe.lookup_visible);
         attempt++) {
        suzaku_ibus_probe_pump_events();
        g_usleep(10000);
    }
    if (!probe.commit_received) {
        result = 9;
    } else if (probe.preedit_visible || probe.lookup_visible) {
        result = 16;
    }
    if (result == 0 && complete_word) {
        if (g_strcmp0(probe.committed, completion_expected) != 0) { result = 21; }
    }

    *candidate_count = probe.candidate_count;
    *page_size = probe.page_size;
    *selected_index = probe.selected_index;

cleanup:
    g_free(completion_expected);
    if (use_global_engine && engine_switched && previous_engine != NULL) {
        if (!ibus_bus_set_global_engine(bus, previous_engine) && result == 0) {
            result = 10;
        }
    }
    ibus_input_context_focus_out(context);
    suzaku_ibus_probe_pump_events();
    g_free(previous_engine);
    g_object_unref(context);
    g_object_unref(bus);
    return result;
}
