"""Keep borrowed IBus signal payloads alive until their C emitter returns.

IBus checks whether each payload is floating AFTER emitting its Python signal.
Some PyGObject versions sink that reference while marshalling, then destroy the
last wrapper as the callback returns. Merely reading text can consequently leave
IBus checking an already freed object. Hold the wrappers until dispatch returns;
do not suppress GLib diagnostics, copy payloads or manually change C refcounts.
This helper belongs to the non-reentrant Python QA observers, not the IME host.
"""


class IBusSignalObjects:
    SIGNALS = ("commit-text", "update-preedit-text", "update-preedit-text-with-mode",
               "update-auxiliary-text", "update-lookup-table", "register-properties",
               "update-property")

    def __init__(self):
        self._pending = []

    def watch(self, context):
        for signal in self.SIGNALS:
            context.connect(signal, self.hold)

    def hold(self, _context, payload, *_arguments):
        # Keep only the payload, not its input context or an unbounded history.
        self._pending.append(payload)

    def drain(self, main_context):
        # Release after EVERY dispatch, including synchronous post-process
        # signals queued before entering this pump. An idle callback could run
        # too early inside a nested main loop, or starve behind busy sources.
        while True:
            dispatched = main_context.iteration(False)
            self._pending.clear()
            if not dispatched:
                return
