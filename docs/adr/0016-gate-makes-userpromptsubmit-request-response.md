# The Prompt Gate makes the UserPromptSubmit exchange request/response

**Context.** Trigger frames are newline-delimited JSON sent one way and fire-and-forget (ADR-0007):
the hook writes a frame and exits, and the host parses and applies it with nothing sent back. The
Prompt Gate (ADR-0014) needs a verdict back before the prompt can proceed, so at least one exchange
has to become request/response. The question is whether to keep the one-way model everywhere else.

**Decision.** Only the `UserPromptSubmit` exchange, and only when the hook runs with `--gate`,
becomes request/response: the hook sends its open intent marked as a gate query and waits, within
its existing bounded timeout, for an allow or block verdict, then either proceeds and lets the open
register or blocks and sends no open. Every other frame, and the gate-off `UserPromptSubmit`, stays
one-way fire-and-forget. The reply is bounded by the hook's write and read timeout and fails open on
no reply. Rejected: making all frames request/response, which would warm every hook event against
ADR-0008 for a reply only the gate needs.

**Consequences.** The host's listener, which today parses and discards frames, gains a reply path for
this one exchange, answered on the single event loop (ADR-0009) so no new concurrency is introduced.
The protocol is no longer purely one-way, but the exception is narrow and opt-in, and the
fire-and-forget model still governs everything else. This extends ADR-0007 rather than replacing it.
Spec: [`docs/specs/prompt-gate.md`](../specs/prompt-gate.md).
