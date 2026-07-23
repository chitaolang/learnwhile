# Milestones

Five milestones from an empty repo to a v1 a developer can install and use daily. Vocabulary
follows the glossary in [`/CONTEXT.md`](../../CONTEXT.md); decisions are cited as ADR-NNNN from
[`docs/adr/`](../adr/); scope is [`docs/specs/v1-trigger-spine-and-learning-engine.md`](../specs/v1-trigger-spine-and-learning-engine.md).

## The rule these follow

Every milestone is a **vertical slice**: it ships UI *and* backend, and it ends with something a
developer can actually run and judge. None of them is "build the Storage module" — a milestone
that leaves you with a module and no way to use it can't be evaluated, and can't be dogfooded.

The order is not arbitrary. It follows the spec's sequencing argument: the spine lands first
because it carries the least reversible risk (does the hook fire where we think it does, does
fail-open really hold), while it is still cheap to change. FSRS and storage are conventional
work that can follow with confidence.

## The five

| # | Milestone | What a developer can do at the end |
|---|---|---|
| [M1](./M1-trigger-spine.md) | Trigger spine | See a card appear in a pane while the agent works, and clear when it returns |
| [M2](./M2-cards-and-reviews.md) | Cards and Reviews | Seed a real deck and complete Reviews that persist and reschedule |
| [M3](./M3-honest-scheduling.md) | Honest scheduling | Trust that what they're shown is genuinely due, capped, and never pulled forward |
| [M4](./M4-session-continuity.md) | Session continuity | Resume a Review interrupted by the agent, and re-attempt a card they failed |
| [M5](./M5-hardening-and-install.md) | Hardening and install | Install it, run it every day, and diagnose it when it misbehaves |

## Reading a milestone

Each file has the same shape:

- **Goal** — one sentence: what becomes possible that wasn't before.
- **Demo** — the literal sequence you perform to see it working. This is the milestone's
  definition of done in the sense that matters; if the demo isn't convincing, the milestone
  isn't finished, whatever the checklist says.
- **What ships** — split into UI and Backend, so neither can quietly be deferred.
- **Sub-tasks** — the work, ordered so that each leaves the build green where practical.
- **Tests** — what must pass, phrased as observable behaviour per the spec's testing decisions.
- **Exit criteria** — binary checks, for deciding whether to move on.
- **Not in this milestone** — deliberately deferred, naming the milestone that picks it up. This
  is what stops a slice from quietly growing into the whole product.
- **Decisions this relies on** — the ADRs that constrain the work.
- **Risks** — what could invalidate the approach, and what to do about it.

## Scaffolding is allowed; silent scaffolding is not

Earlier milestones stand in for later ones in two named places: M1 renders a hardcoded card, and
M2 selects the next card with a placeholder that M3 replaces. Both are called out in the
milestone that introduces them *and* in the one that removes them. Any other stand-in should be
added to this list rather than left for a future reader to discover.
