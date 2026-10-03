/* Presentation ownership is independent of composition subscriptions. A panel
 * claims only after displaying a current public snapshot, and renews while that
 * surface remains usable. Watchers never acquire ownership merely by reading. */
#define SUZAKU_PRESENTATION_LEASE_MS 1200
#define SUZAKU_PRESENTATION_MAX_REQUEST 256
#define SUZAKU_PRESENTATION_MAX_OWNER 64

static gchar *suzaku_presentation_owner = NULL;
static guint64 suzaku_presentation_context = 0;
static gint64 suzaku_presentation_deadline = 0;
static guint suzaku_presentation_source = 0;
static void suzaku_ibus_engine_render_presentation(SuzakuIBusEngine *self);

static void suzaku_presentation_clear(void) {
    if (suzaku_presentation_source != 0) {
        g_source_remove(suzaku_presentation_source);
        suzaku_presentation_source = 0;
    }
    g_clear_pointer(&suzaku_presentation_owner, g_free);
    suzaku_presentation_context = 0;
    suzaku_presentation_deadline = 0;
}

static gboolean suzaku_presentation_eligible(SuzakuIBusEngine *self) {
    return suzaku_ibus_engine_is_focused(IBUS_ENGINE(self)) &&
        !self->private_input && !self->bypass_input && self->input->len > 0 &&
        !suzaku_ibus_engine_is_composing(self);
}

static gboolean suzaku_presentation_active(SuzakuIBusEngine *self) {
    if (suzaku_presentation_owner == NULL) { return FALSE; }
    if (suzaku_presentation_context != suzaku_companion_context ||
        g_get_monotonic_time() >= suzaku_presentation_deadline) {
        suzaku_presentation_clear();
        return FALSE;
    }
    /* A late render of an old engine must not revoke the new field's lease. */
    if (!suzaku_ibus_engine_is_focused(IBUS_ENGINE(self))) { return FALSE; }
    if (!suzaku_presentation_eligible(self)) {
        suzaku_presentation_clear();
        return FALSE;
    }
    return TRUE;
}

static void suzaku_presentation_refresh(void) {
    GObject *focused = g_weak_ref_get(&suzaku_last_focused_engine);
    if (focused != NULL) {
        /* Never publish a frame or touch composition/model state on a lease
         * transition: otherwise every acknowledgement would stale its owner. */
        suzaku_ibus_engine_render_presentation((SuzakuIBusEngine *)focused);
    }
    g_clear_object(&focused);
}

static gboolean suzaku_presentation_expired(gpointer data);

static void suzaku_presentation_schedule(void) {
    if (suzaku_presentation_source != 0) {
        g_source_remove(suzaku_presentation_source);
    }
    gint64 remaining = suzaku_presentation_deadline - g_get_monotonic_time();
    guint delay = (guint)MAX((remaining + G_TIME_SPAN_MILLISECOND - 1) /
        G_TIME_SPAN_MILLISECOND, 1);
    suzaku_presentation_source = g_timeout_add(delay, suzaku_presentation_expired, NULL);
}

static gboolean suzaku_presentation_expired(gpointer data) {
    (void)data;
    suzaku_presentation_source = 0;
    /* All operations share the main context. Still consult the current deadline
     * rather than assuming the timeout's original claim has not been renewed. */
    if (suzaku_presentation_owner != NULL &&
        g_get_monotonic_time() < suzaku_presentation_deadline) {
        suzaku_presentation_schedule();
        return G_SOURCE_REMOVE;
    }
    suzaku_presentation_clear();
    suzaku_presentation_refresh();
    return G_SOURCE_REMOVE;
}

static gboolean suzaku_presentation_number(const gchar *text, guint64 *value) {
    if (*text == '\0') { return FALSE; }
    for (const gchar *p = text; *p != '\0'; p++) {
        if (!g_ascii_isdigit(*p)) { return FALSE; }
    }
    return g_ascii_string_to_unsigned(text, 10, 0, G_MAXUINT64, value, NULL);
}

static gboolean suzaku_presentation_request(const gchar *request, gsize length) {
    if (length < 2 || length >= SUZAKU_PRESENTATION_MAX_REQUEST || request[0] != 'V') {
        return FALSE;
    }
    /* Reject embedded NULs, non-ASCII bytes and control characters before split. */
    for (gsize i = 1; i < length; i++) {
        if ((guchar)request[i] < 0x20 || (guchar)request[i] > 0x7e) { return FALSE; }
    }
    gchar **parts = g_strsplit(request + 1, " ", -1);
    guint64 context = 0, revision = 0;
    gboolean valid = g_strv_length(parts) == 5 &&
        suzaku_companion_host_id != NULL &&
        g_strcmp0(parts[0], suzaku_companion_host_id) == 0 &&
        suzaku_presentation_number(parts[1], &context) &&
        suzaku_presentation_number(parts[2], &revision) &&
        strlen(parts[3]) > 0 && strlen(parts[3]) <= SUZAKU_PRESENTATION_MAX_OWNER &&
        (strcmp(parts[4], "0") == 0 || strcmp(parts[4], "1") == 0);
    if (valid) {
        for (const gchar *p = parts[3]; *p != '\0'; p++) {
            if (!g_ascii_isalnum(*p) && *p != '_' && *p != '-') { valid = FALSE; break; }
        }
    }
    gboolean accepted = FALSE;
    if (valid && parts[4][0] == '0') {
        /* Release may refer to an older revision of the same field. It may not
         * revoke another owner, a new field, or an instance from an old host. */
        if (suzaku_presentation_owner != NULL &&
            context == suzaku_presentation_context &&
            strcmp(parts[3], suzaku_presentation_owner) == 0) {
            suzaku_presentation_clear();
            suzaku_presentation_refresh();
            accepted = TRUE;
        }
    } else if (valid && context == suzaku_companion_context &&
        revision == suzaku_companion_revision) {
        GObject *focused = g_weak_ref_get(&suzaku_last_focused_engine);
        SuzakuIBusEngine *engine = (SuzakuIBusEngine *)focused;
        if (engine != NULL && suzaku_presentation_eligible(engine)) {
            gboolean occupied = suzaku_presentation_active(engine);
            if (!occupied || strcmp(parts[3], suzaku_presentation_owner) == 0) {
                if (!occupied) { suzaku_presentation_owner = g_strdup(parts[3]); }
                suzaku_presentation_context = context;
                suzaku_presentation_deadline = g_get_monotonic_time() +
                    SUZAKU_PRESENTATION_LEASE_MS * G_TIME_SPAN_MILLISECOND;
                suzaku_presentation_schedule();
                suzaku_presentation_refresh();
                accepted = TRUE;
            }
        }
        g_clear_object(&focused);
    }
    g_strfreev(parts);
    return accepted;
}
