# The Runtime tracks an open-Trigger set; "waiting" means it is non-empty

**Context.** Multiple agents share one host, so Triggers from different adapters overlap. If
the Runtime tracked only a single active Trigger, one agent returning would clear the card
while the developer is still waiting on another. Card visibility must reflect aggregate
idleness, not any single agent.

**Decision.** The Runtime maintains a set of currently-open Triggers, keyed by adapter and
agent session. Opening a Trigger adds an entry; closing it (Stop / PermissionRequest /
Elicitation) removes that entry. The developer is "waiting" exactly while the set is
non-empty: a card is surfaced while non-empty and cleared when it becomes empty.

**Consequences.** Runtime state is a set/refcount, not a single slot. Adapters must send a
stable Trigger identity so opens and closes pair up. The host must tolerate a lost close (an
agent or the host crashing mid-Trigger) so the set does not leak a phantom open that keeps a
card up forever — via a per-Trigger timeout and/or a session-end sweep. The rolling Session
spans many non-empty/empty cycles of this set.
