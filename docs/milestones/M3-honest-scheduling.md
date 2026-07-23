# M3 — Honest scheduling

**Goal.** A developer can trust what the pane shows them: a genuinely due card if one exists,
otherwise a new card within the day's cap, otherwise an idle state that explains itself — and
never a card pulled forward ahead of its due date.

## Demo

1. Seed a deck larger than the daily new-card cap and run the host.
2. Submit prompts across several waits. New cards are introduced one at a time until the cap is
   reached, then the pane shows the idle state with counts rather than dumping the deck.
3. Rate a card Good. It does not reappear.
4. The next day (or with the clock advanced in a test), the cap resets and due cards come back
   first, ahead of any new ones.
5. With nothing due and the cap spent, the idle state shows due-today and new-remaining counts
   plus the next due time — enough to tell "nothing due" apart from "not Waiting".

## What ships

**UI.** The idle/stats pane, which is now real content rather than M1's placeholder. Within a
single long wait, rating one card surfaces the next immediately, so a long wait can hold several
Reviews.

**Backend.** The real selection policy replacing M2's `select_next` placeholder, plus the daily
new-card cap and its rollover.

## Sub-tasks

1. **Delete the placeholder.** Replace `select_next` with the ADR-0002 order, evaluated fresh on
   each surfacing: a genuinely due card, else a new card if today's introductions are under the
   cap, else the idle state.
2. **Due query.** "Due" means due at or before the injected clock's now. The query runs against
   the clock, never against SQLite's own time functions — otherwise tests cannot control it.
3. **Daily cap.** Read `new_cards_per_day` from config. "Today" is resolved in the user's local
   timezone via `chrono`'s `Local`, not UTC — a developer's day boundary is their own, and a UTC
   boundary would reset the cap mid-afternoon for some of them.
4. **Counting introductions.** Derive today's count from `review_history` rather than keeping a
   counter, so a restart cannot lose or double it.
5. **Idle state content.** Due-today count, new-remaining count, and the next due time. This is
   what closes the idle-pane gap the design draft carried for so long, so make it genuinely
   informative — a blank pane and an uninformative pane are the same bug.
6. **Next card within a wait.** After a rating persists, re-run selection immediately rather than
   waiting for the next Trigger. A long wait should hold several Reviews.
7. **Empty-deck path.** A deck with no cards at all must render something sensible, not an empty
   frame or a panic.

## Tests

- Selection follows due → new → idle strictly, with a deck constructed to make each branch fire.
- A not-yet-due card is never surfaced, even when the deck contains nothing else and the
  developer is Waiting. This is the ADR-0002 guarantee and the one users cannot verify for
  themselves, so it deserves the most direct test in the milestone.
- The cap holds within a day and rolls over when the clock advances past local midnight.
- The cap survives a restart: introductions already made today still count.
- The idle state shows correct counts, including with an empty deck and with everything due
  already reviewed.
- Rating a card during a wait surfaces the next one without a new Trigger.

## Exit criteria

- Every branch of ADR-0002's order is exercised by a test that observes the pane, not the
  selection function.
- The idle state is informative enough that a developer seeing it does not file a bug.
- Nothing in the codebase can surface a card ahead of its due date. (M4 adds the one bounded
  exception, deliberately and under ADR-0010.)

## Not in this milestone

- The lapse queue — **M4**. Until then, ADR-0002's order holds without exception.
- Session lifecycle and in-flight resumption — **M4**.
- FSRS parameter optimization: out of v1 entirely; defaults only.

## Decisions this relies on

ADR-0002 (selection protects the scheduler), and the config defaults table in the v1 spec.

## Risks

**`new_cards_per_day: 20` is a guess.** The spec says so explicitly. It was chosen because it is
Anki's default, but the right number depends on how many Triggers a developer actually gets in a
day — which is what v1 exists to measure. Do not tune it on a hunch during this milestone; leave
it configurable and revisit it once `review_history` holds real data.

**Local-timezone day boundaries are a common source of off-by-one bugs.** Test the rollover with
the clock set near midnight, and with a non-UTC local timezone, rather than only at noon UTC
where every implementation looks correct.
