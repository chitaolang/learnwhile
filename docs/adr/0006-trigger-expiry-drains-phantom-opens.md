# Open Triggers expire, so a lost close cannot pin the card up forever

**Context.** ADR-0005 makes the developer "Waiting" exactly while the open-Trigger set is
non-empty, and flags that the host must tolerate a lost close without resolving how. A close
can go missing several ways: the agent crashes mid-turn, the host restarts while a Trigger is
open, a hook fails to fire on `Stop`, or an adapter has a bug. Any of these leaks a phantom
open — and because a single leaked entry keeps the set non-empty, the card stays up
permanently and card visibility stops carrying any information at all. A session-end sweep
alone cannot fix this: ADR-0004 makes adapters fire-and-forget with no persistent connection,
so "the adapter disconnected" is not an observable event. An adapter heartbeat is likewise
unavailable, because hooks only run when the agent emits an event — precisely the thing that
is missing during a long turn.

**Decision.** Each entry in the open-Trigger set carries an expiry, measured from when the
Trigger opened, defaulting to 30 minutes and stored in `config` as `trigger_expiry_seconds`
rather than hardcoded. A sweep runs on a periodic timer, independent of frame arrival, and
drops expired entries; if that empties the set, the card clears exactly as a real close would.
Expiry is deliberately *not* refreshed on subsequent frames: with only open and close frames
defined (ADR-0007), no traffic exists mid-turn to refresh from, so a refresh rule would be
inert complexity. Should adapters later emit intermediate progress frames, refreshing becomes
meaningful and can be revisited.

**Consequences.** The two failure directions are asymmetric, which is what justifies erring
short: expiring early clears a card while the developer is still waiting — mildly annoying,
self-corrects on the next Trigger — whereas expiring late or never pins a stale card up
indefinitely and silently destroys the meaning of the surface. A genuinely uninterrupted agent
turn longer than 30 minutes will drop its card mid-wait; that is accepted, and the config key
exists for developers whose agents routinely run longer. The sweep must be driven by its own
timer: hanging it off frame arrival would mean a phantom open with no subsequent traffic — the
exact case this ADR addresses — never drains. Because expiry is time-based rather than
liveness-based, the host does not need to track adapter processes at all.
