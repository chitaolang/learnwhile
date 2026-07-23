# A lapsed card returns within the same Session; the ban on pulling forward holds across them

**Context.** ADR-0002 forbids ever surfacing a not-yet-due card, to keep FSRS intervals honest.
That rule was written before the scheduler's API surface was confirmed, and the confirmation
changes what it implies. The `fsrs` crate models long-term memory only: `next_states` returns
day-granularity intervals over a `MemoryState` of stability and difficulty, with no learning
steps and no sub-day scheduling. Under ADR-0002 read literally, a card the developer rates
Again is scheduled days out and cannot be shown before then — so failing a card and passing it
produce the same experience for the rest of the day. That sits badly against the premise in
DESIGN_DRAFT §3: LearnWhile's unit is not a daily sitting but many short waits inside one
rolling Session, and the minutes right after a failed recall are the ones where re-attempting
it is worth most.

**Decision.** A card rated Again joins an in-memory lapse queue owned by the current Session.
Selection order becomes: the lapse queue, else a genuinely due card, else a new card under the
daily cap, else the idle state. A re-attempt is an ordinary Review in every other respect — it
calls `next_states` with `days_elapsed` of zero, persists the returned state, and appends a
`review_history` row, where the zero elapsed days is what distinguishes an intra-Session repeat
in the audit trail. A card leaves the queue when rated anything other than Again. The queue is
never persisted: it dies with the Session and with the host process. Rejected: **honouring the
FSRS interval literally**, which keeps ADR-0002 intact at the cost of the review experience
described above; **implementing Anki-style learning steps**, which means running a second
scheduler beside FSRS and owning the interaction between the two — precisely the work choosing
an upstream crate was meant to avoid; and **persisting the lapse queue**, which sounds like a
robustness improvement but converts a Session-local affordance into durable state capable of
pulling cards forward across days, which is the thing ADR-0002 exists to prevent.

**Consequences.** This narrows ADR-0002 rather than reversing it, and the narrowing is the
load-bearing part: the ban on early review still holds absolutely across Sessions and across
days, and the exception is bounded by the Session, by the card having been failed in that
Session, and by the queue living only in memory. Within a Session, a failed card can be
surfaced before its persisted due date; that is now intended behaviour, not a bug. FSRS sees
the re-attempt as a review with zero elapsed days, which the crate accepts; whether that helps
or hurts parameter fit is unmeasurable until the Analytics Engine exists, and v1 runs on
default parameters without any fitting pass, so nothing rides on it yet. A crash loses the
queue and the affected cards simply revert to their persisted due dates — a fail-open outcome
consistent with the rest of the system. The `cards.state` column keeps a domain of `new` and
`review` in v1; lapsed is deliberately not one of its values, because being in the queue is
Session state and not a property of the card.
