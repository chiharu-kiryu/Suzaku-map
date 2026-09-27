/* A keyboard gesture owns a focus/context-bound asynchronous settings job.
 * Unlike an explicit tray language change, it reinterprets the latest draft
 * without submitting or erasing it. It never grabs a desktop-wide shortcut. */
static void suzaku_ibus_cancel_language_change(SuzakuIBusEngine *self) {
    if (self->language_source != 0) {
        g_source_remove(self->language_source);
        self->language_source = 0;
    }
    suzaku_host_ime_control_free(self->language_job);
    self->language_job = NULL;
}

static void suzaku_ibus_language_feedback(SuzakuIBusEngine *self, const gchar *message) {
    g_free(self->language_feedback);
    self->language_feedback = g_strdup(message);
    suzaku_ibus_engine_render(self);
}

static gboolean suzaku_ibus_language_ready(gpointer data) {
    SuzakuIBusEngine *self = data;
    if (!suzaku_ibus_engine_is_focused(IBUS_ENGINE(self)) || self->private_input ||
        self->language_context != suzaku_companion_context || suzaku_ibus_engine_is_composing(self)) {
        suzaku_ibus_cancel_language_change(self);
        g_clear_pointer(&self->language_feedback, g_free);
        return G_SOURCE_REMOVE;
    }
    if (g_get_monotonic_time() >= self->language_deadline) {
        suzaku_ibus_cancel_language_change(self);
        suzaku_ibus_language_feedback(self, "语言切换超时，未保存；请稍后重试");
        return G_SOURCE_REMOVE;
    }
    char *response = suzaku_host_ime_control_poll_utf8(self->language_job);
    if (response == NULL) { return G_SOURCE_CONTINUE; }
    self->language_source = 0;
    suzaku_host_ime_control_free(self->language_job);
    self->language_job = NULL;
    if (strstr(response, "\"ok\":true") != NULL) {
        /* Revoke old-language panel/tool actions, but retain text typed while
         * the durable write was pending. Only old adoption undo/Compose reset.
         * Do not clear held keys: a still-held Space must not toggle twice or
         * insert a separator when its modifiers are released first. */
        suzaku_companion_context++;
        g_clear_pointer(&self->language_feedback, g_free);
        suzaku_ibus_engine_sync_input(self);
        suzaku_ibus_language_feedback(self, "已切换输入语言，草稿保留 · Ctrl+Shift+Space");
    } else {
        suzaku_ibus_language_feedback(self, strstr(response, "正在保存") != NULL ?
            "设置正在保存，语言切换未保存；请稍后重试" :
            "语言切换未保存；请从托盘检查或重新加载设置后重试");
    }
    suzaku_host_ime_free_utf8(response);
    return G_SOURCE_REMOVE;
}

static void suzaku_ibus_start_language_change(SuzakuIBusEngine *self) {
    if (self->language_job != NULL) {
        suzaku_ibus_language_feedback(self, "正在切换输入语言，请稍候");
        return;
    }
    const gchar *command = suzaku_host_ime_language_kind() == 2 ? "Lzh-Hans" : "Len";
    self->language_context = suzaku_companion_context;
    self->language_deadline = g_get_monotonic_time() + G_TIME_SPAN_SECOND;
    self->language_job = suzaku_host_ime_control_start_utf8(command, 1000);
    if (self->language_job == NULL) {
        suzaku_ibus_language_feedback(self, "语言切换未保存；请稍后重试");
        return;
    }
    self->language_source = g_timeout_add(5, suzaku_ibus_language_ready, self);
    suzaku_ibus_language_feedback(self, "正在切换输入语言…");
}
