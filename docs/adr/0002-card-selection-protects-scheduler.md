# Card selection never early-reviews; it protects the scheduler

**Context.** Triggers surface cards during random idle windows, but FSRS (the chosen
scheduler, for Anki compatibility) assumes cards are reviewed around when they are due.
Reviewing a not-yet-due card early feeds the scheduler off-schedule data and blunts the
spacing benefit. Yet the product's premise is to *use* the waiting time, so showing
"nothing" on most triggers is also a failure.

**Decision.** On each Trigger the Learning Engine selects in strict order: (1) a genuinely
due card; else (2) introduce a new/unseen card, up to a daily new-card cap; else (3) show a
neutral idle/stats state. It never pulls a not-yet-due card forward. This uses waiting time
via new-card introduction while keeping every scheduled interval honest.

**Consequences.** With a small deck or few due cards, some waits legitimately show the idle
state — this is intended, not a bug. A daily new-card cap is required to avoid dumping a
whole deck in one busy day. Choosing FSRS over SM-2 follows the "Anki-compatible" goal;
switching schedulers later would be a separate, larger decision.
