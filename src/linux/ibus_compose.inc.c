/* Per-engine Compose/dead-key handling. Tables are immutable and loaded once
 * during engine construction, never on the key-event path. Honor the user's
 * locale/XCompose without changing the system layout or global locale. */
static struct xkb_compose_state *suzaku_ibus_compose_new(void) {
    static gsize initialized = 0;
    static struct xkb_compose_table *table = NULL;
    if (g_once_init_enter(&initialized)) {
        struct xkb_context *context = xkb_context_new(XKB_CONTEXT_NO_FLAGS);
        if (context != NULL) {
            const gchar *locale = g_getenv("LC_ALL");
            if (locale == NULL || *locale == '\0') { locale = g_getenv("LC_CTYPE"); }
            if (locale == NULL || *locale == '\0') { locale = g_getenv("LANG"); }
            if (locale == NULL || *locale == '\0' ||
                strcmp(locale, "C") == 0 || strcmp(locale, "POSIX") == 0) {
                locale = "en_US.UTF-8";
            }
            table = xkb_compose_table_new_from_locale(
                context, locale, XKB_COMPOSE_COMPILE_NO_FLAGS);
            if (table == NULL && strcmp(locale, "en_US.UTF-8") != 0) {
                table = xkb_compose_table_new_from_locale(
                    context, "en_US.UTF-8", XKB_COMPOSE_COMPILE_NO_FLAGS);
            }
            xkb_context_unref(context);
        }
        if (table == NULL) {
            g_printerr("Suzaku could not load a Compose table; dead-key composition is unavailable.\n");
        }
        /* The shared table lives for the host process; each engine owns only
         * its state, which is reset at field boundaries and freed at finalize. */
        g_once_init_leave(&initialized, 1);
    }
    return table == NULL ? NULL :
        xkb_compose_state_new(table, XKB_COMPOSE_STATE_NO_FLAGS);
}

static void suzaku_ibus_engine_cancel_compose(SuzakuIBusEngine *self) {
    if (suzaku_ibus_engine_is_composing(self)) {
        suzaku_ibus_engine_reset_compose(self);
        suzaku_ibus_engine_render(self);
    }
}

static gboolean suzaku_ibus_engine_process_compose(SuzakuIBusEngine *self, guint keyval) {
    if (self->compose == NULL ||
        xkb_compose_state_feed(self->compose, keyval) == XKB_COMPOSE_FEED_IGNORED) {
        return FALSE;
    }
    switch (xkb_compose_state_get_status(self->compose)) {
        case XKB_COMPOSE_NOTHING:
            return FALSE;
        case XKB_COMPOSE_COMPOSING:
            g_clear_pointer(&self->completion_undo, g_free);
            suzaku_ibus_engine_render(self);
            return TRUE;
        case XKB_COMPOSE_COMPOSED: {
            gchar text[256];
            gint length = xkb_compose_state_get_utf8(self->compose, text, sizeof(text));
            suzaku_ibus_engine_reset_compose(self);
            gboolean clean = length > 0 && (gsize)length < sizeof(text) &&
                g_utf8_validate(text, length, NULL);
            for (const gchar *p = text; clean && *p != '\0'; p = g_utf8_next_char(p)) {
                if (!g_unichar_isprint(g_utf8_get_char(p))) { clean = FALSE; }
            }
            if (clean) {
                g_string_append_len(self->input, text, length);
                suzaku_ibus_engine_sync_input(self);
            } else {
                /* Do not truncate, inject controls, or reinterpret the final
                 * key as an unrelated number choice or submit operation. */
                suzaku_ibus_engine_render(self);
            }
            return TRUE;
        }
        case XKB_COMPOSE_CANCELLED: {
            suzaku_ibus_engine_reset_compose(self);
            gunichar character = ibus_keyval_to_unicode(keyval);
            if (character != 0 && g_unichar_isprint(character)) {
                /* Preserve the cancelling printable key literally. In
                 * particular a digit must not unexpectedly select a word. */
                suzaku_ibus_engine_append_character(self, character);
                return TRUE;
            }
            suzaku_ibus_engine_render(self);
            return FALSE;
        }
    }
    return FALSE;
}
