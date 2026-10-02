/* Model IBus's floating signal ownership without an IME, display or bus. */
#include <glib-object.h>

static unsigned live_payloads;

static void finalized(gpointer data, GObject *object) {
    (void)data;
    (void)object;
    --live_payloads;
}

unsigned borrowed_signal_live(void) {
    return live_payloads;
}

gboolean borrowed_signal_emit(GObject *emitter) {
    GObject *payload = g_object_new(G_TYPE_INITIALLY_UNOWNED, NULL);
    ++live_payloads;
    g_object_weak_ref(payload, finalized, NULL);
    unsigned before = live_payloads;
    g_signal_emit_by_name(emitter, "borrowed-object", payload);
    /* Detect early destruction without dereferencing a freed object, unlike
     * IBus's post-signal is_floating check. This must remain a test failure. */
    if (live_payloads != before) {
        return FALSE;
    }
    if (g_object_is_floating(payload)) {
        g_object_ref_sink(payload);
        g_object_unref(payload);
    }
    return TRUE;
}
