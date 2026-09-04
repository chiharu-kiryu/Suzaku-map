#include <ibus.h>
#include <glib/gstdio.h>
#include <stdbool.h>
#include <stddef.h>
#include <string.h>

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
extern char *suzaku_host_ime_take_last_committed_text_utf8(void);
extern void suzaku_host_ime_free_utf8(char *text);

typedef struct _SuzakuIBusEngine {
    IBusEngine parent_instance;
    GString *input;
} SuzakuIBusEngine;

typedef struct _SuzakuIBusEngineClass {
    IBusEngineClass parent_class;
} SuzakuIBusEngineClass;

G_DEFINE_TYPE(SuzakuIBusEngine, suzaku_ibus_engine, IBUS_TYPE_ENGINE)

static GWeakRef suzaku_last_focused_engine;
static GSocketService *suzaku_ipc_service = NULL;
static gchar *suzaku_ipc_socket_path = NULL;

static void suzaku_ibus_engine_render(SuzakuIBusEngine *self) {
    IBusEngine *engine = IBUS_ENGINE(self);
    if (self->input->len == 0) {
        ibus_engine_hide_preedit_text(engine);
        ibus_engine_hide_lookup_table(engine);
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
        return;
    }

    size_t selected = suzaku_host_ime_selected_index();
    IBusLookupTable *table = ibus_lookup_table_new(
        9, (guint)MIN(selected, candidate_count - 1), TRUE, FALSE);
    for (size_t index = 0; index < candidate_count; index++) {
        char *label = suzaku_host_ime_candidate_label_utf8(index);
        if (label == NULL) {
            continue;
        }
        ibus_lookup_table_append_candidate(
            table, ibus_text_new_from_string(label));
        suzaku_host_ime_free_utf8(label);
    }
    ibus_engine_update_lookup_table(engine, table, TRUE);
}

static void suzaku_ibus_engine_sync_input(SuzakuIBusEngine *self) {
    if (self->input->len == 0) {
        suzaku_host_ime_clear_marked_text();
    } else {
        suzaku_host_ime_replace_marked_text_utf8(self->input->str);
    }
    suzaku_ibus_engine_render(self);
}

static void suzaku_ibus_engine_clear(SuzakuIBusEngine *self) {
    g_string_truncate(self->input, 0);
    suzaku_host_ime_clear_marked_text();
    suzaku_ibus_engine_render(self);
}

static gboolean suzaku_ibus_engine_commit(SuzakuIBusEngine *self) {
    if (self->input->len == 0 || !suzaku_host_ime_commit_selected(true)) {
        return FALSE;
    }

    char *committed = suzaku_host_ime_take_last_committed_text_utf8();
    if (committed == NULL || committed[0] == '\0') {
        suzaku_host_ime_free_utf8(committed);
        return FALSE;
    }

    ibus_engine_commit_text(
        IBUS_ENGINE(self), ibus_text_new_from_string(committed));
    suzaku_host_ime_free_utf8(committed);
    suzaku_ibus_engine_clear(self);
    return TRUE;
}

static void suzaku_ibus_engine_move_selection(
    SuzakuIBusEngine *self, ptrdiff_t delta) {
    if (self->input->len == 0) {
        return;
    }
    suzaku_host_ime_move_selection(delta);
    suzaku_ibus_engine_render(self);
}

static gboolean suzaku_ibus_engine_process_key_event(
    IBusEngine *engine, guint keyval, guint keycode, guint state) {
    (void)keycode;
    SuzakuIBusEngine *self = (SuzakuIBusEngine *)engine;

    if ((state & IBUS_RELEASE_MASK) != 0) {
        return FALSE;
    }
    if ((state & (IBUS_CONTROL_MASK | IBUS_MOD1_MASK | IBUS_SUPER_MASK)) != 0) {
        return FALSE;
    }

    if (keyval == IBUS_KEY_BackSpace && self->input->len > 0) {
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
    if (keyval == IBUS_KEY_Up || keyval == IBUS_KEY_Page_Up) {
        if (self->input->len == 0) {
            return FALSE;
        }
        suzaku_ibus_engine_move_selection(
            self, keyval == IBUS_KEY_Page_Up ? -9 : -1);
        return TRUE;
    }
    if (keyval == IBUS_KEY_Down || keyval == IBUS_KEY_Page_Down) {
        if (self->input->len == 0) {
            return FALSE;
        }
        suzaku_ibus_engine_move_selection(
            self, keyval == IBUS_KEY_Page_Down ? 9 : 1);
        return TRUE;
    }
    if (keyval == IBUS_KEY_space || keyval == IBUS_KEY_Return ||
        keyval == IBUS_KEY_KP_Enter) {
        return suzaku_ibus_engine_commit(self);
    }

    if (self->input->len > 0 && keyval >= IBUS_KEY_1 && keyval <= IBUS_KEY_9) {
        size_t index = (size_t)(keyval - IBUS_KEY_1);
        if (index < suzaku_host_ime_candidate_count()) {
            suzaku_host_ime_select_candidate(index);
            return suzaku_ibus_engine_commit(self);
        }
    }

    gunichar character = ibus_keyval_to_unicode(keyval);
    if ((character >= 'a' && character <= 'z') ||
        (character >= 'A' && character <= 'Z') || character == '\'') {
        gchar utf8[7] = {0};
        character = g_unichar_tolower(character);
        gint length = g_unichar_to_utf8(character, utf8);
        g_string_append_len(self->input, utf8, length);
        suzaku_ibus_engine_sync_input(self);
        return TRUE;
    }

    if (self->input->len > 0) {
        suzaku_ibus_engine_commit(self);
    }
    return FALSE;
}

static void suzaku_ibus_engine_focus_in(IBusEngine *engine) {
    SuzakuIBusEngine *self = (SuzakuIBusEngine *)engine;
    g_weak_ref_set(&suzaku_last_focused_engine, G_OBJECT(engine));
    suzaku_host_ime_activate();
    suzaku_ibus_engine_sync_input(self);
}

static void suzaku_ibus_clear_focused_engine_if(IBusEngine *engine) {
    GObject *focused = g_weak_ref_get(&suzaku_last_focused_engine);
    if (focused == G_OBJECT(engine)) {
        g_weak_ref_set(&suzaku_last_focused_engine, NULL);
    }
    g_clear_object(&focused);
}

static void suzaku_ibus_engine_focus_out(IBusEngine *engine) {
    SuzakuIBusEngine *self = (SuzakuIBusEngine *)engine;
    suzaku_ibus_clear_focused_engine_if(engine);
    suzaku_ibus_engine_clear(self);
    suzaku_host_ime_deactivate();
}

static void suzaku_ibus_engine_reset(IBusEngine *engine) {
    suzaku_ibus_engine_clear((SuzakuIBusEngine *)engine);
}

static void suzaku_ibus_engine_enable(IBusEngine *engine) {
    (void)engine;
    suzaku_host_ime_activate();
}

static void suzaku_ibus_engine_disable(IBusEngine *engine) {
    suzaku_ibus_clear_focused_engine_if(engine);
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
    suzaku_ibus_engine_move_selection((SuzakuIBusEngine *)engine, -9);
}

static void suzaku_ibus_engine_page_down(IBusEngine *engine) {
    suzaku_ibus_engine_move_selection((SuzakuIBusEngine *)engine, 9);
}

static void suzaku_ibus_engine_candidate_clicked(
    IBusEngine *engine, guint index, guint button, guint state) {
    (void)button;
    (void)state;
    SuzakuIBusEngine *self = (SuzakuIBusEngine *)engine;
    if (self->input->len == 0 || index >= suzaku_host_ime_candidate_count()) {
        return;
    }
    suzaku_host_ime_select_candidate(index);
    suzaku_ibus_engine_commit(self);
}

static void suzaku_ibus_engine_finalize(GObject *object) {
    SuzakuIBusEngine *self = (SuzakuIBusEngine *)object;
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
}

static void suzaku_ibus_engine_init(SuzakuIBusEngine *self) {
    self->input = g_string_new(NULL);
}

static void suzaku_ibus_bus_disconnected(IBusBus *bus, gpointer user_data) {
    (void)bus;
    (void)user_data;
    ibus_quit();
}

static gboolean suzaku_ibus_ipc_incoming(
    GSocketService *service,
    GSocketConnection *connection,
    GObject *source_object,
    gpointer user_data) {
    (void)service;
    (void)source_object;
    (void)user_data;

    gchar request[65536];
    gsize bytes_read = 0;
    GError *error = NULL;
    g_socket_set_timeout(g_socket_connection_get_socket(connection), 1);
    GInputStream *input = g_io_stream_get_input_stream(G_IO_STREAM(connection));
    gboolean read_ok = g_input_stream_read_all(
        input, request, sizeof(request) - 1, &bytes_read, NULL, &error);
    if (!read_ok && error != NULL) {
        g_warning("Suzaku IPC read failed: %s", error->message);
        g_clear_error(&error);
    }
    request[MIN(bytes_read, sizeof(request) - 1)] = '\0';

    gboolean delivered = FALSE;
    if (bytes_read > 1 && request[0] == 'C' &&
        g_utf8_validate(request + 1, (gssize)bytes_read - 1, NULL)) {
        GObject *focused = g_weak_ref_get(&suzaku_last_focused_engine);
        if (focused != NULL) {
            IBusEngine *engine = IBUS_ENGINE(focused);
            ibus_engine_commit_text(
                engine, ibus_text_new_from_string(request + 1));
            g_object_unref(focused);
            delivered = TRUE;
        }
    } else if (bytes_read == 1 && request[0] == 'Q') {
        delivered = TRUE;
    }

    const gchar response = delivered ? '1' : '0';
    GOutputStream *output = g_io_stream_get_output_stream(G_IO_STREAM(connection));
    gsize bytes_written = 0;
    if (!g_output_stream_write_all(
            output, &response, 1, &bytes_written, NULL, &error) &&
        error != NULL) {
        g_warning("Suzaku IPC response failed: %s", error->message);
        g_clear_error(&error);
    }
    g_output_stream_close(output, NULL, NULL);
    return TRUE;
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

    suzaku_ibus_stop_ipc_service();
    g_weak_ref_clear(&suzaku_last_focused_engine);
    g_object_unref(factory);
    g_object_unref(bus);
    return 0;
}

typedef struct {
    gchar *committed;
    gsize capacity;
    gboolean received;
} SuzakuIBusProbe;

static void suzaku_ibus_probe_commit_text(
    IBusInputContext *context, IBusText *text, gpointer user_data) {
    (void)context;
    SuzakuIBusProbe *probe = (SuzakuIBusProbe *)user_data;
    const gchar *value = ibus_text_get_text(text);
    if (value == NULL || value[0] == '\0') {
        return;
    }
    g_strlcpy(probe->committed, value, probe->capacity);
    probe->received = TRUE;
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
    char *committed,
    size_t committed_capacity) {
    if (engine_name == NULL || seed == NULL || seed[0] == '\0' ||
        committed == NULL || committed_capacity == 0) {
        return 1;
    }
    committed[0] = '\0';

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
        .capacity = committed_capacity,
        .received = FALSE,
    };
    g_signal_connect(context, "commit-text",
                     G_CALLBACK(suzaku_ibus_probe_commit_text), &probe);
    ibus_input_context_set_capabilities(
        context, IBUS_CAP_FOCUS | IBUS_CAP_PREEDIT_TEXT | IBUS_CAP_LOOKUP_TABLE);
    ibus_input_context_focus_in(context);
    suzaku_ibus_probe_pump_events();

    int result = 0;
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

    if (via_ipc) {
        if (!suzaku_ibus_probe_send_ipc_commit(seed)) {
            result = 11;
            goto cleanup;
        }
    } else {
        for (const gchar *cursor = seed; *cursor != '\0';
             cursor = g_utf8_next_char(cursor)) {
            gunichar character = g_utf8_get_char(cursor);
            if (!ibus_input_context_process_key_event(context, character, 0, 0)) {
                result = 7;
                goto cleanup;
            }
        }
        if (!ibus_input_context_process_key_event(context, IBUS_KEY_space, 0, 0)) {
            result = 8;
            goto cleanup;
        }
    }

    for (guint attempt = 0; attempt < 100 && !probe.received; attempt++) {
        suzaku_ibus_probe_pump_events();
        g_usleep(10000);
    }
    if (!probe.received) {
        result = 9;
    }

cleanup:
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
