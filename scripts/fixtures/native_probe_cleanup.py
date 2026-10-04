"""Small lifecycle guards for owned, non-reentrant native probe fixtures."""
from contextlib import contextmanager
import json
from pathlib import Path
import sys
import time


class PendingCallbacks:
    def __init__(self, glib):
        self._glib = glib
        self._sources = set()
        self._failure = None
        self._failure_notes = 0

    def schedule(self, delay_ms, callback):
        def invoke():
            try:
                callback()
            except Exception as error:
                # PyGObject prints callback exceptions instead of propagating
                # them out of iteration(). The owning test must check explicitly.
                if self._failure is None:
                    self._failure = error
                elif self._failure_notes < 4:
                    note = (f"Additional delayed callback failure: {type(error).__name__}: {str(error)[:1024]}"
                            if self._failure_notes < 3 else "Further delayed callback failures omitted.")
                    self._failure.add_note(note)
                    self._failure_notes += 1
            finally:
                # Do not later cancel a retired/reused source ID.
                self._sources.discard(source_id)
            return self._glib.SOURCE_REMOVE

        source_id = self._glib.timeout_add(delay_ms, invoke)
        self._sources.add(source_id)
        return source_id

    def cancel_all(self):
        while self._sources:
            self._glib.source_remove(self._sources.pop())

    def raise_if_failed(self):
        if self._failure is not None:
            raise self._failure


@contextmanager
def preserve_primary_failure(label):
    """Use inside finally: attach cleanup errors without replacing a test failure."""
    primary = sys.exc_info()[1]
    try:
        yield
    except Exception as cleanup_error:
        if not isinstance(primary, Exception):
            # In particular, SystemExit(0) is not a failed assertion and must
            # never conceal unsuccessful cleanup behind a successful exit.
            raise
        if cleanup_error is not primary:
            primary.add_note(f"{label}: {type(cleanup_error).__name__}: {cleanup_error}")


def report_native_diagnostic(event, *, phase, started, bus, daemon, processes, **details):
    """Report owned process/local connection state without D-Bus RPC or full argv."""
    try:
        connection = bus.get_connection() if bus is not None else None
        children = [{"name": Path(process.args[0]).name, "pid": process.pid,
                     "returncode": process.poll()} for process in processes[-8:]]
        state = {"event": event, "phase": phase,
                 "elapsed_s": round(time.monotonic() - started, 3),
                 "bus_connected": bus.is_connected() if bus is not None else None,
                 "connection_closed": connection.is_closed() if connection is not None else None,
                 "daemon_pid": daemon.pid if daemon is not None else None,
                 "daemon_returncode": daemon.poll() if daemon is not None else None,
                 "recent_children": children, **details}
        print("NATIVE DIAGNOSTIC: " + json.dumps(state), file=sys.stderr, flush=True)
    except Exception as error:
        # Neither a broken connection accessor nor an unavailable stderr may
        # replace the input/connection failure this report is meant to explain.
        try:
            print(f"NATIVE DIAGNOSTIC unavailable: {type(error).__name__}: {str(error)[:1024]}",
                  file=sys.stderr, flush=True)
        except Exception:
            pass
