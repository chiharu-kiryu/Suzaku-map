#!/usr/bin/env python3
"""Compile the real probe teardown against deterministic Gio-linked stubs.

No IBus, GI, display, D-Bus daemon or network connection is started. --baseline
extracts HEAD without changing the checkout and is expected to fail the contract
before the pending-call barrier fix.
"""
import argparse
from pathlib import Path
import shlex
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SOURCE = "src/linux/ibus_engine_bridge.c"
BASELINE = False


def extract_function(source, signature):
    if source.count(signature) != 1:
        raise AssertionError(f"expected exactly one {signature!r}")
    start = source.index(signature)
    end = source.index("\n}\n", start) + len("\n}\n")
    return source[start:end]


STUBS = r'''
#include <gio/gio.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define REQUIRE(condition) do { \
    if (!(condition)) { \
        fprintf(stderr, "contract failed at line %d: %s\n", __LINE__, #condition); \
        exit(1); \
    } \
} while (0)

typedef struct { int marker; } IBusBus;
static IBusBus bus_token, context_token;
static int connection_token;
static const char *mode;
static gboolean finalized, pending_remove_match, acknowledged, bus_closed;
static unsigned connection_calls, closed_checks, rpc_calls, reply_frees, error_clears, error_frees;

static GDBusConnection *fixture_connection(IBusBus *bus) {
    REQUIRE(bus == &bus_token && finalized && !bus_closed);
    connection_calls++;
    return strcmp(mode, "null") == 0 ? NULL : (GDBusConnection *)&connection_token;
}

static gboolean fixture_is_closed(GDBusConnection *connection) {
    REQUIRE(connection == (GDBusConnection *)&connection_token);
    closed_checks++;
    return strcmp(mode, "closed") == 0;
}

static GVariant *fixture_call_sync(
    GDBusConnection *connection, const gchar *destination, const gchar *path,
    const gchar *interface, const gchar *method, GVariant *parameters,
    const GVariantType *reply_type, GDBusCallFlags flags, gint timeout,
    GCancellable *cancellable, GError **error) {
    REQUIRE(connection == (GDBusConnection *)&connection_token);
    REQUIRE(finalized && pending_remove_match && !bus_closed);
    REQUIRE(strcmp(mode, "null") != 0 && strcmp(mode, "closed") != 0);
    REQUIRE(g_strcmp0(destination, "org.freedesktop.DBus") == 0);
    REQUIRE(g_strcmp0(path, "/org/freedesktop/DBus") == 0);
    REQUIRE(g_strcmp0(interface, "org.freedesktop.DBus") == 0);
    REQUIRE(g_strcmp0(method, "GetNameOwner") == 0);
    REQUIRE(g_variant_is_of_type(parameters, G_VARIANT_TYPE("(s)")));
    const gchar *name = NULL;
    g_variant_get(parameters, "(&s)", &name);
    REQUIRE(g_strcmp0(name, "org.freedesktop.DBus") == 0);
    REQUIRE(g_variant_type_equal(reply_type, G_VARIANT_TYPE("(s)")));
    REQUIRE(flags == G_DBUS_CALL_FLAGS_NONE && timeout == 250);
    REQUIRE(cancellable == NULL && error != NULL && *error == NULL);
    rpc_calls++;
    /* The real call consumes floating parameters. Keep actual Gio variants. */
    g_variant_ref_sink(parameters);
    g_variant_unref(parameters);
    if (strcmp(mode, "rpc-failure") == 0) {
        g_set_error_literal(error, G_IO_ERROR, G_IO_ERROR_TIMED_OUT, "synthetic ACK timeout");
        return NULL;
    }
    pending_remove_match = FALSE;
    acknowledged = TRUE;
    return g_variant_ref_sink(g_variant_new("(s)", ":1.42"));
}

static void fixture_variant_unref(GVariant *reply) {
    REQUIRE(!bus_closed && acknowledged && reply != NULL);
    reply_frees++;
    g_variant_unref(reply);
}

static void fixture_clear_error(GError **error) {
    REQUIRE(!bus_closed && error != NULL);
    error_clears++;
    if (*error != NULL) {
        REQUIRE((*error)->domain == G_IO_ERROR && (*error)->code == G_IO_ERROR_TIMED_OUT);
        error_frees++;
    }
    g_clear_error(error);
    REQUIRE(*error == NULL);
}

static void fixture_object_unref(gpointer object) {
    if (object == &context_token) {
        REQUIRE(!finalized && !bus_closed);
        /* Proxy finalization, not Destroy, queues its asynchronous RemoveMatch. */
        finalized = TRUE;
        pending_remove_match = TRUE;
    } else {
        REQUIRE(object == &bus_token && finalized && !bus_closed);
        if (strcmp(mode, "success") == 0) {
            REQUIRE(acknowledged && !pending_remove_match);
            REQUIRE(reply_frees == 1 && error_clears == 1);
        }
        bus_closed = TRUE;
    }
}

#define ibus_bus_get_connection fixture_connection
#define g_dbus_connection_is_closed fixture_is_closed
#define g_dbus_connection_call_sync fixture_call_sync
#define g_variant_unref fixture_variant_unref
#define g_clear_error fixture_clear_error
#undef g_object_unref
#define g_object_unref fixture_object_unref
'''

MAIN = r'''
int main(int argc, char **argv) {
    REQUIRE(argc == 3);
    mode = argv[1];
    int original = atoi(argv[2]);
    REQUIRE(strcmp(mode, "success") == 0 || strcmp(mode, "null") == 0 ||
            strcmp(mode, "closed") == 0 || strcmp(mode, "rpc-failure") == 0);
    int result = run_cleanup(&bus_token, &context_token, original);
    gboolean success = strcmp(mode, "success") == 0;
    gboolean called = success || strcmp(mode, "rpc-failure") == 0;
    REQUIRE(result == (success || original != 0 ? original : 23));
    REQUIRE(finalized && bus_closed && connection_calls == 1);
    REQUIRE(closed_checks == (strcmp(mode, "null") == 0 ? 0u : 1u));
    REQUIRE(rpc_calls == (called ? 1u : 0u));
    REQUIRE(reply_frees == (success ? 1u : 0u));
    REQUIRE(error_clears == (called ? 1u : 0u));
    REQUIRE(error_frees == (strcmp(mode, "rpc-failure") == 0 ? 1u : 0u));
    REQUIRE(acknowledged == success && pending_remove_match != success);
    printf("PASS: %s original=%d result=%d; context finalizes before barrier and bus close\n",
           mode, original, result);
    return 0;
}
'''


class NativeProbeTeardownTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if BASELINE:
            source = subprocess.run(["git", "show", f"HEAD:{SOURCE}"], cwd=ROOT,
                                    capture_output=True, text=True, check=True, timeout=5).stdout
        else:
            source = (ROOT / SOURCE).read_text()
        signature = "static gboolean suzaku_ibus_probe_finish_pending_calls(IBusBus *bus) {"
        helper = extract_function(source, signature) if signature in source else ""
        if not BASELINE:
            assert helper, "pending-call helper missing from current source"
        roundtrip = extract_function(source, "int suzaku_linux_ibus_probe_roundtrip(")
        start = roundtrip.rindex("    g_object_unref(context);\n")
        end = roundtrip.index("    return result;\n", start) + len("    return result;\n")
        # Compile the exact final cleanup block, including its error-stage logic.
        cleanup = roundtrip[start:end]
        harness = STUBS + helper + "\nstatic int run_cleanup(IBusBus *bus, void *context, int result) {\n"
        harness += cleanup + "}\n" + MAIN
        cls.temporary = tempfile.TemporaryDirectory(prefix="suzaku-probe-teardown.", dir="/tmp")
        cls.addClassCleanup(cls.temporary.cleanup)
        path = Path(cls.temporary.name)
        (path / "teardown.c").write_text(harness)
        cls.binary = path / "teardown"
        flags = subprocess.run(["pkg-config", "--cflags", "--libs", "gio-2.0"],
                               capture_output=True, text=True, check=True, timeout=5)
        # HEAD has no helper, so its unused stubs must still compile for a red test.
        subprocess.run(["cc", "-std=c11", "-Wall", "-Wextra", "-Werror",
                        "-Wno-unused-function", str(path / "teardown.c"), "-o", str(cls.binary),
                        *shlex.split(flags.stdout)], check=True, timeout=15)

    def test_teardown_order_and_error_preservation(self):
        for mode in ["success", "null", "closed", "rpc-failure"]:
            for original in [0, 7, 10]:
                with self.subTest(mode=mode, original=original):
                    result = subprocess.run([str(self.binary), mode, str(original)],
                                            capture_output=True, text=True, timeout=2)
                    self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                    self.assertIn(f"PASS: {mode} original={original}", result.stdout)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", action="store_true", help="test HEAD without changing the checkout")
    options, remaining = parser.parse_known_args()
    BASELINE = options.baseline
    unittest.main(argv=[sys.argv[0], *remaining])
