# A Session is the lifetime of the host process

**Context.** The lapse queue (ADR-0010) and the in-flight Review are Session-scoped: they live in
memory and must die with the Session, reverting affected cards to their persisted due dates. So the
code needs to know when a Session ends. The glossary ties a Session to "a Trigger Adapter being
connected (roughly a work sitting)," but that is a description of the concept, not a condition the
host can test. ADR-0004 makes adapters fire-and-forget with no persistent connection, and ADR-0006
already established that adapter liveness is not observable — hooks fire only when the agent emits
an event, which is precisely the signal missing between waits. "Connected" is therefore not
something the host can detect, so a Session boundary has to be defined against something it can.

**Decision.** In v1, a Session is exactly the lifetime of the host process: it begins when the host
starts and ends when the process exits. The lapse queue and the in-flight Review are ordinary
in-memory fields of the running host and need no explicit start or end logic, because process start
and exit already bound them; nothing about a Session is written to disk. Rejected: an
**idle-timeout Session** that ends after the open-Trigger set has been empty for some minutes, which
is closer to the "sitting" the glossary evokes but adds a timer, a policy constant, and a discard
path — machinery ADR-0010's affordance does not need in v1, and a knob with no data to tune it, the
same objection the daily new-card cap already carries. Rejected: **a Session per Waiting span**,
tying it to the open-Trigger set being non-empty, which would discard the lapse queue every time the
agent returned — the exact opposite of ADR-0010's intent, since a lapsed card has to survive the
wait that follows in order to be re-offered at all.

**Consequences.** The README already tells developers to start the host once per sitting and leave
it, so process lifetime approximates a sitting in practice. A host restart is a new Session: the
lapse queue is gone and affected cards sit at their persisted due dates — the behaviour the M4
restart test pins down, and a fail-open outcome consistent with the rest of the system. Because the
boundary is process lifetime, there is no Session-end code to get wrong and no timer to serialise
onto the single event loop (ADR-0009). The cost is that two sittings inside one long-lived host
share a Session: a card failed in the morning is still queued in the afternoon if the host was never
restarted. That is accepted for v1. A finer-grained Session can replace this later without touching
ADR-0010, because "the queue dies with the Session" stays true whatever bounds the Session; should
that finer boundary become load-bearing, it earns its own record rather than a quiet change here.
