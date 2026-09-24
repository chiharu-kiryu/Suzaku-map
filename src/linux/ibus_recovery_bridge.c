/* A separate connection/context owned exclusively by the recovery thread.
 * IBus uses well-known names as signal senders; use its native GIO transport.
 * Only engine names and host ownership are read, never field contents. */
#include <gio/gio.h>
#include <stdbool.h>
#include <stddef.h>
#include <string.h>

#define IBUS "org.freedesktop.IBus"
#define IBUS_PATH "/org/freedesktop/IBus"
#define COMPONENT "org.freedesktop.IBus.Suzaku"
#define DBUS "org.freedesktop.DBus"
#define DBUS_PATH "/org/freedesktop/DBus"
#define PROPERTIES "org.freedesktop.DBus.Properties"

typedef void (*RecoverySignal)(guint, const gchar *, const gchar *, void *);
typedef struct {
    GMainContext *context;
    GDBusConnection *connection;
    guint engine_subscription, owner_subscription;
    RecoverySignal callback;
    void *data;
    gboolean connected;
} RecoveryBus;

static GVariant *call(RecoveryBus *bus, const gchar *destination,
                     const gchar *path, const gchar *interface,
                     const gchar *method, GVariant *args,
                     const GVariantType *type, GError **error) {
    return g_dbus_connection_call_sync(bus->connection, destination, path,
        interface, method, args, type, G_DBUS_CALL_FLAGS_NONE, 400, NULL, error);
}

static GVariant *property(RecoveryBus *bus, const gchar *path,
                          const gchar *interface, const gchar *name, GError **error) {
    GVariant *reply = call(bus, IBUS, path, PROPERTIES, "Get",
        g_variant_new("(ss)", interface, name), G_VARIANT_TYPE("(v)"), error);
    if (reply == NULL) { return NULL; }
    GVariant *value = NULL;
    g_variant_get(reply, "(v)", &value);
    g_variant_unref(reply);
    return value;
}

static gboolean missing(GError *error, const gchar *message) {
    if (!g_error_matches(error, G_DBUS_ERROR, G_DBUS_ERROR_FAILED)) { return FALSE; }
    g_dbus_error_strip_remote_error(error);
    return g_strcmp0(error->message, message) == 0;
}

static void signal_received(GDBusConnection *connection, const gchar *sender,
                            const gchar *path, const gchar *interface,
                            const gchar *member, GVariant *args, gpointer data) {
    (void)connection; (void)sender; (void)path; (void)interface;
    RecoveryBus *bus = data;
    if (g_strcmp0(member, "GlobalEngineChanged") == 0 &&
        g_variant_is_of_type(args, G_VARIANT_TYPE("(s)"))) {
        const gchar *engine = NULL;
        g_variant_get(args, "(&s)", &engine);
        bus->callback(1, engine, "", bus->data);
    } else if (g_strcmp0(member, "NameOwnerChanged") == 0 &&
               g_variant_is_of_type(args, G_VARIANT_TYPE("(sss)"))) {
        const gchar *name = NULL, *old = NULL, *next = NULL;
        g_variant_get(args, "(&s&s&s)", &name, &old, &next);
        if (g_strcmp0(name, COMPONENT) == 0) { bus->callback(2, old, next, bus->data); }
    }
}

static void connected(GObject *source, GAsyncResult *result, gpointer data) {
    (void)source;
    RecoveryBus *bus = data;
    GError *error = NULL;
    bus->connection = g_dbus_connection_new_for_address_finish(result, &error);
    g_clear_error(&error);
    bus->connected = TRUE;
}

static gboolean cancel_connect(gpointer data) {
    g_cancellable_cancel(data);
    return G_SOURCE_REMOVE;
}

void suzaku_ibus_recovery_free(RecoveryBus *bus) {
    if (bus->connection != NULL) {
        if (bus->engine_subscription) {
            g_dbus_connection_signal_unsubscribe(bus->connection, bus->engine_subscription);
        }
        if (bus->owner_subscription) {
            g_dbus_connection_signal_unsubscribe(bus->connection, bus->owner_subscription);
        }
        g_dbus_connection_close(bus->connection, NULL, NULL, NULL);
        g_object_unref(bus->connection);
    }
    g_main_context_pop_thread_default(bus->context);
    g_main_context_unref(bus->context);
    g_free(bus);
}

RecoveryBus *suzaku_ibus_recovery_new(
    const gchar *address, RecoverySignal callback, void *data) {
    RecoveryBus *bus = g_new0(RecoveryBus, 1);
    bus->context = g_main_context_new();
    bus->callback = callback;
    bus->data = data;
    g_main_context_push_thread_default(bus->context);
    GCancellable *cancel = g_cancellable_new();
    GSource *timeout = g_timeout_source_new(400);
    g_source_set_callback(timeout, cancel_connect, cancel, NULL);
    g_source_attach(timeout, bus->context);
    g_dbus_connection_new_for_address(address,
        G_DBUS_CONNECTION_FLAGS_AUTHENTICATION_CLIENT | G_DBUS_CONNECTION_FLAGS_MESSAGE_BUS_CONNECTION,
        NULL, cancel, connected, bus);
    while (!bus->connected) { g_main_context_iteration(bus->context, TRUE); }
    g_source_destroy(timeout);
    g_source_unref(timeout);
    g_object_unref(cancel);
    if (bus->connection == NULL) { suzaku_ibus_recovery_free(bus); return NULL; }
    g_dbus_connection_set_exit_on_close(bus->connection, FALSE);
    GVariant *reply = call(bus, IBUS, IBUS_PATH, IBUS, "GetUseGlobalEngine",
                          NULL, G_VARIANT_TYPE("(b)"), NULL);
    gboolean global = FALSE;
    if (reply != NULL) { g_variant_get(reply, "(b)", &global); g_variant_unref(reply); }
    if (!global) { suzaku_ibus_recovery_free(bus); return NULL; }
    bus->engine_subscription = g_dbus_connection_signal_subscribe(bus->connection,
        IBUS, IBUS, "GlobalEngineChanged", IBUS_PATH, NULL, 0, signal_received, bus, NULL);
    bus->owner_subscription = g_dbus_connection_signal_subscribe(bus->connection,
        DBUS, DBUS, "NameOwnerChanged", DBUS_PATH, COMPONENT, 0, signal_received, bus, NULL);
    return bus;
}

bool suzaku_ibus_recovery_poll(RecoveryBus *bus) {
    /* Bound each drain too: shutdown must not be starved by queued signals. */
    for (guint count = 0; count < 128; count++) {
        if (!g_main_context_iteration(bus->context, FALSE)) { break; }
    }
    return !g_dbus_connection_is_closed(bus->connection);
}

/* 1 = confirmed value, 0 = confirmed absent, -1 = uncertain/error.
 * Never reinterpret a failed/timed-out property query as an absent engine. */
int suzaku_ibus_recovery_query(RecoveryBus *bus, guint operation, gchar *out, size_t capacity) {
    GError *error = NULL;
    GVariant *value = NULL;
    const gchar *name = NULL;
    int result = -1;
    if (operation == 0) {
        value = call(bus, DBUS, DBUS_PATH, DBUS, "GetNameOwner",
            g_variant_new("(s)", COMPONENT), G_VARIANT_TYPE("(s)"), &error);
        if (value != NULL) { g_variant_get(value, "(&s)", &name); }
        else if (g_error_matches(error, G_DBUS_ERROR, G_DBUS_ERROR_NAME_HAS_NO_OWNER)) { result = 0; }
    } else if (operation == 1) {
        value = property(bus, IBUS_PATH, IBUS, "GlobalEngine", &error);
        if (value == NULL && missing(error, "No global engine.")) { result = 0; }
        for (guint depth = 0; value != NULL && depth < 2 &&
             g_variant_is_of_type(value, G_VARIANT_TYPE_VARIANT); depth++) {
            GVariant *inner = g_variant_get_variant(value);
            g_variant_unref(value);
            value = inner;
        }
        if (value != NULL && g_variant_is_of_type(value, G_VARIANT_TYPE_TUPLE) &&
            g_variant_n_children(value) >= 3) {
            GVariant *kind = g_variant_get_child_value(value, 0);
            GVariant *engine = g_variant_get_child_value(value, 2);
            if (g_variant_is_of_type(kind, G_VARIANT_TYPE_STRING) &&
                g_strcmp0(g_variant_get_string(kind, NULL), "IBusEngineDesc") == 0 &&
                g_variant_is_of_type(engine, G_VARIANT_TYPE_STRING)) {
                name = g_variant_get_string(engine, NULL);
                if (strlen(name) < capacity) { g_strlcpy(out, name, capacity); result = 1; }
            }
            g_variant_unref(kind);
            g_variant_unref(engine);
            name = NULL;
        }
    }
    if (name != NULL && strlen(name) < capacity) { g_strlcpy(out, name, capacity); result = 1; }
    g_clear_pointer(&value, g_variant_unref);
    g_clear_error(&error);
    return result;
}

void suzaku_ibus_recovery_select(RecoveryBus *bus, const gchar *name) {
    GVariant *reply = call(bus, IBUS, IBUS_PATH, IBUS, "SetGlobalEngine",
        g_variant_new("(s)", name), G_VARIANT_TYPE("()"), NULL);
    g_clear_pointer(&reply, g_variant_unref);
}
