# Adapters send newline-delimited JSON frames identified by adapter and session

**Context.** ADR-0004 settled the transport (a unix domain socket) and explicitly left the
message format open. The format has to satisfy several things at once: it must be trivial to
emit from a short-lived hook process, carry a stable Trigger identity so opens and closes pair
up (ADR-0005), survive garbage or partial input without taking down the host, and leave room
for the host to reply later when an opt-in Prompt Gate needs an answer on the same channel.
Volume is negligible — a handful of frames per agent turn — so wire efficiency is close to
irrelevant, while debuggability and ease of writing a non-Rust adapter are not.

**Decision.** One UTF-8 JSON object per line, newline-terminated:

``` json
{ "v": 1, "type": "trigger_open", "adapter": "claude-code", "session": "<agent-session-id>", "at": "<rfc3339>" }
```

`type` is `trigger_open` or `trigger_close`. Trigger identity is the `(adapter, session)` pair.
The host reads line-at-a-time under a bounded maximum line length, ignores frames whose `v` it
does not recognise, and ignores any line it cannot parse — in no case may a bad frame kill the
accept loop. Rejected: length-prefixed binary or `bincode`, which is faster but unreadable on
the wire and hostile to adapters written in another language; protobuf or msgpack, whose schema
tooling is disproportionate to two message types; and bare positional text, which has no room
to grow a field without breaking every adapter.

**Consequences.** The format is verbose relative to the information carried, which does not
matter at this volume. A maximum line length is mandatory rather than defensive polish: without
it a buggy or hostile client can stream an unterminated line and exhaust host memory. Silently
ignoring malformed input keeps the fail-open posture intact but makes adapter bugs invisible,
so the host needs a log file to make discarded frames diagnosable — the pane cannot serve this
purpose, since it must stay passive (ADR-0001). Because the framing is symmetric, a future
Prompt Gate reply reuses it with a new `type` and no transport work. The `v` field costs one
key and is what allows a future adapter to be built against a frozen contract.
