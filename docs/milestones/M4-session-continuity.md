# M4 — Session continuity

**Goal.** A developer's review survives the rhythm of real agent use: a card they were halfway
through when the agent came back is still there on the next wait, and a card they failed comes
back for a second attempt before the sitting ends.

## Demo

1. Run the host and submit a prompt. A card appears; reveal it but do not rate it.
2. Let the agent come back. The pane clears.
3. Submit another prompt. The *same* card is back, still revealed, still waiting for a rating.
4. Rate it Again.
5. Keep working. On a later wait in the same sitting, that card is offered again, ahead of due
   and new cards.
6. Rate it Good. It stops coming back for the rest of the sitting.
7. Restart the host. The lapse queue is gone and the card sits at its persisted due date.

## What ships

**UI.** Continuity, which is felt rather than seen: the pane resumes mid-Review instead of
restarting it, and re-offers failed cards. Worth making the resumed state legible so a developer
returning to a revealed answer understands why they are looking at it.

**Backend.** The Session lifecycle, in-flight Review state that outlives a pane clear, and the
lapse queue from ADR-0010.

## Sub-tasks

1. **Session lifecycle.** A rolling Session, tied to a Trigger Adapter being connected rather
   than to any single wait, spanning many Waiting/idle cycles of the open-Trigger set. Define
   when a Session starts and ends and write it down — the glossary says what a Session *is*, but
   its start and end conditions are the thing the code has to decide.
2. **In-flight card survives the clear.** Clearing the pane when the open-Trigger set empties
   must not discard the Review state machine's position. This is a state-ownership change, not a
   rendering change: the Renderer stops drawing, Learning keeps its state.
3. **Resume on the next Trigger.** Re-surface the in-flight card in the state it was left in —
   a revealed answer stays revealed, so the developer is not asked to recall something they have
   already seen.
4. **Lapse queue.** In-memory, Session-scoped. A card rated Again is appended; a card rated
   anything else is removed (ADR-0010).
5. **Selection order.** Insert the lapse queue at the front: lapse → due → new → idle. This
   narrows M3's implementation of ADR-0002 and should cite ADR-0010 where it does, so the next
   reader does not "fix" it back.
6. **Re-attempts are ordinary Reviews.** Call `next_states` with `days_elapsed` of zero and
   persist as usual. The resulting `review_history` row with zero elapsed days is what identifies
   an intra-Session repeat in the audit trail; no extra column.
7. **Abandonment.** A card can be left unfinished indefinitely. Ignoring LearnWhile during a wait
   carries no penalty and no nagging — no timeout on the in-flight card, no re-prompting.
8. **Discard on Session end.** The queue and the in-flight card die with the Session and with the
   process; nothing is written to disk. Affected cards revert to their persisted due dates.

## Tests

- A card revealed but unrated, interrupted by the agent returning, is re-surfaced on the next
  Trigger in the same revealed state.
- A card rated Again returns on a later Trigger in the same Session, ahead of due and new cards.
- That card stops returning once rated Good — re-attempts converge rather than looping.
- The lapse queue does not survive a host restart: reboot the harness against the same database
  and the card is at its persisted due date, not in the queue.
- Rating Again still writes a `review_history` row and still reschedules — a Lapse is a completed
  Review, not a skipped one.
- An unfinished card left across many Waiting/idle cycles is neither discarded nor escalated.

## Exit criteria

- The demo's step 3 works with a real agent, not just in the harness — this is the behaviour most
  likely to be subtly wrong under real timing.
- No lapse state reaches SQLite. Grep the schema: being in the queue is Session state, not card
  state, and `cards.state` keeps its `{new, review}` domain.
- ADR-0002's ban still holds across Sessions and days; the exception is bounded to the Session.

## Not in this milestone

- Logging, single-instance detection, install polish — **M5**.
- Persisting the lapse queue: explicitly rejected by ADR-0010, because it would turn a
  Session-local affordance into durable state that pulls cards forward across days.
- Learning Contracts and Prompt Gates: deferred past v1 entirely. Nothing blocks.

## Decisions this relies on

ADR-0010 (lapses re-queue within the Session), ADR-0002 as narrowed by it, ADR-0005 (the
Waiting edges that trigger surface and clear), and the `Lapse` and `Session` glossary entries.

## Risks

**"When does a Session end?" is genuinely unsettled.** The glossary ties it to a Trigger Adapter
being connected, but ADR-0004 makes adapters fire-and-forget with no persistent connection —
so "connected" is not directly observable, exactly as ADR-0006 notes for liveness. Expect to
define Session end as something time-based or process-based instead. If the answer turns out to
be load-bearing, it deserves an ADR rather than a quiet choice in code.

**Resumption interacts with expiry.** A card can be in flight while its Trigger expires
(ADR-0006). Expiry clears the pane; it must not discard the in-flight card, since the developer
may well still be waiting. Test the two together, because each is easy to get right alone.
