# Adapters reach the host over a unix socket; fail-open by construction

**Context.** ADR-0003 leaves the IPC transport open. "Fail-open" is a core principle: a down
or slow host must never stall the user's agent, and a hook that blocks or errors would do
exactly that — so the transport choice *is* the fail-open mechanism. We also want opt-in
Prompt Gates (deferred from v1) to be cheap to add later, which needs a channel the host can
answer back on.

**Decision.** A Trigger Adapter connects to the long-lived host over a unix domain socket at
a known path (e.g. `$XDG_RUNTIME_DIR/learnwhile.sock`). The hook sends the Trigger event
fire-and-forget with a tight connect/send timeout and swallows all errors (always exits 0).
A missing or refused socket returns instantly, so a down host is a silent no-op. The host is
the sole binder of the socket, aligning with its sole ownership of SQLite. Because the socket
is bidirectional, the same channel can later carry a Review result back to gate the agent for
an opt-in Prompt Gate — no second channel needed.

**Consequences.** The host runs a socket-accept loop alongside the TUI and must unlink a
stale socket on startup. A send timeout is still required to stay fail-open against a wedged-
but-alive host (connect-failure alone is instant). Unix sockets are not native on older
Windows, so a future Windows renderer would need an alternate transport. Chosen over a
watched state file (simpler but one-way, so gating would need a second reverse channel) and
over localhost HTTP (more moving parts: port, server, timeout).
