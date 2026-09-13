/* Included after the engine operations. Pushes are nonblocking and bounded:
 * slow/broken readers are disconnected, never allowed to stall native typing.
 * They reconnect and receive a full current snapshot, not a replay of history. */
static char *suzaku_companion_snapshot(void) {
    if (suzaku_companion_host_id == NULL) { return NULL; }
    GObject *focused = g_weak_ref_get(&suzaku_last_focused_engine);
    gboolean private_input = focused != NULL &&
        ((SuzakuIBusEngine *)focused)->private_input;
    char *frame = suzaku_host_ime_companion_snapshot_utf8(
        suzaku_companion_host_id, suzaku_companion_context,
        suzaku_companion_revision, focused != NULL, private_input);
    g_clear_object(&focused);
    return frame;
}

static gboolean suzaku_companion_send(GSocketConnection *connection, const char *frame) {
    GSocket *socket = g_socket_connection_get_socket(connection);
    gsize length = strlen(frame);
    GError *error = NULL;
    gssize written = g_socket_send(socket, frame, length, NULL, &error);
    g_clear_error(&error);
    if (written != (gssize)length) {
        g_io_stream_close(G_IO_STREAM(connection), NULL, NULL);
        return FALSE;
    }
    return TRUE;
}

static void suzaku_companion_publish(void) {
    suzaku_companion_revision++;
    if (suzaku_companion_subscribers == NULL || suzaku_companion_subscribers->len == 0) { return; }
    char *frame = suzaku_companion_snapshot();
    if (frame == NULL) { return; }
    for (guint i = suzaku_companion_subscribers->len; i > 0; i--) {
        if (!suzaku_companion_send(g_ptr_array_index(suzaku_companion_subscribers, i - 1), frame)) {
            g_ptr_array_remove_index(suzaku_companion_subscribers, i - 1);
        }
    }
    suzaku_host_ime_free_utf8(frame);
}

static void suzaku_companion_subscribe(GSocketConnection *connection) {
    if (suzaku_companion_subscribers == NULL) { return; }
    // A reconnecting panel can replace a dead reader even without input events.
    char *frame = suzaku_companion_snapshot();
    if (frame == NULL) { return; }
    for (guint i = suzaku_companion_subscribers->len; i > 0; i--) {
        if (!suzaku_companion_send(g_ptr_array_index(suzaku_companion_subscribers, i - 1), frame)) {
            g_ptr_array_remove_index(suzaku_companion_subscribers, i - 1);
        }
    }
    if (suzaku_companion_subscribers->len >= 8) {
        g_io_stream_close(g_ptr_array_index(suzaku_companion_subscribers, 0), NULL, NULL);
        g_ptr_array_remove_index(suzaku_companion_subscribers, 0);
    }
    g_socket_set_timeout(g_socket_connection_get_socket(connection), 0);
    g_socket_set_blocking(g_socket_connection_get_socket(connection), FALSE);
    if (suzaku_companion_send(connection, frame)) {
        g_ptr_array_add(suzaku_companion_subscribers, g_object_ref(connection));
    }
    suzaku_host_ime_free_utf8(frame);
}

static gboolean suzaku_companion_action(const char *request) {
    gchar **parts = g_strsplit(request, " ", 3);
    guint64 revision = 0;
    gboolean valid = g_strv_length(parts) == 3 &&
        g_strcmp0(parts[0], suzaku_companion_host_id) == 0 &&
        g_ascii_string_to_unsigned(parts[1], 10, 0, G_MAXUINT64, &revision, NULL) &&
        revision == suzaku_companion_revision;
    GObject *focused = g_weak_ref_get(&suzaku_last_focused_engine);
    SuzakuIBusEngine *engine = (SuzakuIBusEngine *)focused;
    gboolean applied = FALSE;
    if (valid && engine != NULL && !engine->private_input) {
        const char *action = parts[2];
        guint64 index = 0;
        if (engine->input->len > 0 && (action[0] == 'K' || action[0] == 'N') &&
            g_ascii_string_to_unsigned(action + 1, 10, 0, G_MAXUINT64, &index, NULL) &&
            index < suzaku_host_ime_candidate_count()) {
            g_clear_pointer(&engine->completion_undo, g_free);
            suzaku_ibus_engine_reset_compose(engine);
            suzaku_host_ime_select_candidate((size_t)index);
            if (action[0] == 'K') { applied = suzaku_ibus_engine_commit(engine); }
            else { suzaku_ibus_engine_render(engine); applied = TRUE; }
        } else if (strcmp(action, "X") == 0) {
            suzaku_ibus_engine_clear(engine); applied = TRUE;
        } else if (action[0] == 'T' && strlen(action + 1) <= 8192) {
            gboolean clean = TRUE;
            for (const gchar *p = action + 1; *p != '\0'; p = g_utf8_next_char(p)) {
                if (g_unichar_iscntrl(g_utf8_get_char(p))) { clean = FALSE; break; }
            }
            if (clean) {
                g_string_assign(engine->input, action + 1);
                suzaku_ibus_engine_sync_input(engine); applied = TRUE;
            }
        }
    }
    g_clear_object(&focused);
    g_strfreev(parts);
    return applied;
}
