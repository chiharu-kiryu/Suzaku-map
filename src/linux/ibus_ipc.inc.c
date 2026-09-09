/* Main-context transport: no blocking reads/writes, and engine mutations stay
 * on the IBus thread. The wire format remains one EOF-delimited request. */
#define SUZAKU_IPC_MAX_REQUEST 65535
#define SUZAKU_IPC_MAX_CLIENTS 16
#define SUZAKU_IPC_DEADLINE_MS 1000
#define SUZAKU_IPC_DISPATCH_BUDGET 16384

typedef struct {
    GSocketConnection *connection;
    GByteArray *input;
    gchar *output;
    gsize output_length;
    gsize output_offset;
    GSource *io_source;
    GSource *deadline_source;
    gint64 deadline;
    guint64 context;
} SuzakuIpcClient;

static GPtrArray *suzaku_ipc_clients = NULL;
static void suzaku_ibus_ipc_dispatch(SuzakuIpcClient *client);

static void suzaku_ipc_destroy_source(GSource **source) {
    if (*source != NULL) {
        g_source_destroy(*source);
        g_source_unref(*source);
        *source = NULL;
    }
}

static void suzaku_ipc_finish(SuzakuIpcClient *client, gboolean close_connection) {
    suzaku_ipc_destroy_source(&client->io_source);
    suzaku_ipc_destroy_source(&client->deadline_source);
    g_ptr_array_remove_fast(suzaku_ipc_clients, client);
    if (close_connection) {
        g_io_stream_close(G_IO_STREAM(client->connection), NULL, NULL);
    }
    g_object_unref(client->connection);
    g_byte_array_unref(client->input);
    g_free(client->output);
    g_free(client);
}

static gboolean suzaku_ipc_expired(gpointer data) {
    suzaku_ipc_finish(data, TRUE);
    return G_SOURCE_REMOVE;
}

static gboolean suzaku_ipc_write_ready(GSocket *socket, GIOCondition condition, gpointer data) {
    (void)condition;
    SuzakuIpcClient *client = data;
    if (g_get_monotonic_time() >= client->deadline) {
        suzaku_ipc_finish(client, TRUE);
        return G_SOURCE_REMOVE;
    }
    GError *error = NULL;
    gssize written = g_socket_send(socket, client->output + client->output_offset,
        MIN(client->output_length - client->output_offset, SUZAKU_IPC_DISPATCH_BUDGET), NULL, &error);
    gboolean blocked = g_error_matches(error, G_IO_ERROR, G_IO_ERROR_WOULD_BLOCK);
    g_clear_error(&error);
    if (blocked) { return G_SOURCE_CONTINUE; }
    if (written <= 0) {
        suzaku_ipc_finish(client, TRUE);
        return G_SOURCE_REMOVE;
    }
    client->output_offset += (gsize)written;
    if (client->output_offset == client->output_length) {
        suzaku_ipc_finish(client, TRUE);
        return G_SOURCE_REMOVE;
    }
    return G_SOURCE_CONTINUE;
}

static void suzaku_ipc_reply(SuzakuIpcClient *client, const gchar *response) {
    client->output = g_strdup(response);
    client->output_length = strlen(response);
    suzaku_ipc_destroy_source(&client->io_source);
    client->io_source = g_socket_create_source(g_socket_connection_get_socket(client->connection),
        G_IO_OUT | G_IO_ERR | G_IO_HUP, NULL);
    g_source_set_callback(client->io_source, G_SOURCE_FUNC(suzaku_ipc_write_ready), client, NULL);
    g_source_attach(client->io_source, NULL);
}

static gboolean suzaku_ipc_read_ready(GSocket *socket, GIOCondition condition, gpointer data) {
    (void)condition;
    SuzakuIpcClient *client = data;
    for (gsize processed = 0; processed < SUZAKU_IPC_DISPATCH_BUDGET;) {
        if (g_get_monotonic_time() >= client->deadline) {
            suzaku_ipc_finish(client, TRUE);
            return G_SOURCE_REMOVE;
        }
        gchar chunk[4096];
        GError *error = NULL;
        gssize received = g_socket_receive(socket, chunk, sizeof(chunk), NULL, &error);
        gboolean blocked = g_error_matches(error, G_IO_ERROR, G_IO_ERROR_WOULD_BLOCK);
        g_clear_error(&error);
        if (blocked) { return G_SOURCE_CONTINUE; }
        if (received < 0) {
            suzaku_ipc_finish(client, TRUE);
            return G_SOURCE_REMOVE;
        }
        if (received == 0) {
            suzaku_ibus_ipc_dispatch(client);
            return G_SOURCE_REMOVE;
        }
        if (client->input->len + (gsize)received > SUZAKU_IPC_MAX_REQUEST) {
            // Never execute a valid-looking prefix of an oversized request.
            suzaku_ipc_reply(client, "0");
            return G_SOURCE_REMOVE;
        }
        g_byte_array_append(client->input, (const guint8 *)chunk, (guint)received);
        processed += (gsize)received;
    }
    return G_SOURCE_CONTINUE;
}

static gboolean suzaku_ibus_ipc_incoming(GSocketService *service,
    GSocketConnection *connection, GObject *source_object, gpointer user_data) {
    (void)service;
    (void)source_object;
    (void)user_data;
    GSocket *socket = g_socket_connection_get_socket(connection);
    GCredentials *credentials = g_socket_get_credentials(socket, NULL);
    gboolean same_user = credentials != NULL &&
        g_credentials_get_unix_user(credentials, NULL) == getuid();
    g_clear_object(&credentials);
    if (suzaku_ipc_clients == NULL) { suzaku_ipc_clients = g_ptr_array_new(); }
    if (!same_user || suzaku_ipc_clients->len >= SUZAKU_IPC_MAX_CLIENTS) {
        g_io_stream_close(G_IO_STREAM(connection), NULL, NULL);
        return TRUE;
    }
    g_socket_set_timeout(socket, 0);
    g_socket_set_blocking(socket, FALSE);
    SuzakuIpcClient *client = g_new0(SuzakuIpcClient, 1);
    client->connection = g_object_ref(connection);
    client->input = g_byte_array_new();
    client->context = suzaku_companion_context;
    client->deadline = g_get_monotonic_time() + SUZAKU_IPC_DEADLINE_MS * G_TIME_SPAN_MILLISECOND;
    g_ptr_array_add(suzaku_ipc_clients, client);
    client->deadline_source = g_timeout_source_new(SUZAKU_IPC_DEADLINE_MS);
    g_source_set_callback(client->deadline_source, suzaku_ipc_expired, client, NULL);
    g_source_attach(client->deadline_source, NULL);
    client->io_source = g_socket_create_source(socket, G_IO_IN | G_IO_ERR | G_IO_HUP, NULL);
    g_source_set_callback(client->io_source, G_SOURCE_FUNC(suzaku_ipc_read_ready), client, NULL);
    g_source_attach(client->io_source, NULL);
    return TRUE;
}

static void suzaku_ipc_close_clients(void) {
    if (suzaku_ipc_clients == NULL) { return; }
    while (suzaku_ipc_clients->len > 0) {
        suzaku_ipc_finish(g_ptr_array_index(suzaku_ipc_clients, suzaku_ipc_clients->len - 1), TRUE);
    }
    g_clear_pointer(&suzaku_ipc_clients, g_ptr_array_unref);
}
