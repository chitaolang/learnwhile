# M2 — Cards and Reviews

**Goal.** A developer can seed a deck from a file and complete real Reviews during their waits —
question, reveal, rate — with every rating persisted and the card rescheduled by FSRS.

## Demo

1. Write a TSV file with a few front/back pairs and run `learnwhile seed cards.tsv`.
2. Run the host and submit a prompt to Claude Code. The question side of a real card appears.
3. Press the reveal key. The answer appears.
4. Press a rating key. The card is recorded and the pane moves on.
5. Re-run the seed command. Nothing is duplicated.
6. Restart the host and query the SQLite file directly. The Review is there, and the card's due
   date has moved.

## What ships

**UI.** Real card content, question side first. A reveal key. Four rating keys — Again, Hard,
Good, Easy — with the available keys visible on screen, since a developer mid-wait should not
have to remember them.

**Backend.** SQLite with the v1 schema and migrations; the `seed` subcommand; the `fsrs` crate
wired into a Review state machine that persists on every rating.

## Sub-tasks

1. **Storage module.** `rusqlite` with the `bundled` feature. The database path resolves under
   the XDG data directory. This module is the only one issuing SQL.
2. **Migration.** One migration against `PRAGMA user_version`, run on host startup, creating
   `cards`, `decks`, `review_history`, and `config`, and seeding the default deck plus the
   config defaults from the spec's schema table.
3. **Move Trigger expiry into config.** M1 hardcoded 1800; read `trigger_expiry_seconds` from
   `config` now that a database exists, as ADR-0006 requires.
4. **`seed` subcommand.** Parse TSV front/back pairs, compute the content hash, insert into the
   default deck, skip rows whose hash already exists. It should refuse to grow: no CSV, no JSON,
   no Anki formats — that is Import/Export, which is deferred.
5. **Clock injection.** The `Clock` trait and its real implementation, threaded into Learning.
   Not optional: FSRS is time-dependent and due-date behaviour tested against a real clock is
   non-deterministic.
6. **FSRS integration.** Construct `FSRS` with `DEFAULT_PARAMETERS`, read `desired_retention`
   from config, and call `next_states(current, retention, days_elapsed)`. Map the four ratings
   onto the four returned `ItemState`s, then persist the chosen `MemoryState` and interval.
7. **Review state machine.** `Question → Answer → persist → Idle`, driven by key events off the
   channel. A Review is complete only once persisted; correctness is not required. Persist on the
   keypress, not on some later flush — a crash must not lose a rating.
8. **`review_history` write.** Append-only, recording rating, stability and difficulty before and
   after, elapsed days, and scheduled days — enough for the deferred Analytics Engine to
   reconstruct scheduler state.
9. **Placeholder selection.** A named `select_next` seam returning any unreviewed card, so the
   flow is usable end to end. **This is deliberate scaffolding and M3 replaces it.** Keep it to
   a handful of lines, and do not let due-date logic accumulate inside it.
10. **Render real cards.** Replace M1's hardcoded card with the selected one, keeping the pane's
    Waiting and idle states intact.

## Tests

- Seeding a file inserts the cards; seeding the same file again inserts nothing.
- A full reveal-and-rate flow writes exactly one `review_history` row and advances the card's
  due date.
- Each of the four ratings persists, including Again — a Review counts as complete regardless of
  correctness.
- A rating survives a host restart: reboot the harness against the same temp database and the row
  is still there.
- Migration runs cleanly on an empty database and is idempotent on an existing one.
- The question side renders without the answer visible until the reveal key arrives. This is the
  one that protects the product's premise, so assert on the buffer contents rather than trusting
  the state machine.

## Exit criteria

- A developer can go from an empty machine to a completed, persisted Review using only the README.
- FSRS interval maths is not re-tested here — that belongs to the upstream crate. We test that we
  call it and persist what it returns.
- `select_next` is still small enough to delete in one sitting.

## Not in this milestone

- Due-vs-new selection, the daily cap, the stats pane — **M3**.
- Lapses and Session continuity — **M4**. Rating Again reschedules per FSRS and the card does not
  come back today; that is a known deficiency this milestone accepts.
- Manual card entry: out of v1 entirely.

## Decisions this relies on

ADR-0003 (host is sole owner of the SQLite file), ADR-0006 (expiry lives in config), ADR-0009
(key events arrive as channel events, which is what makes the flow testable without a terminal).

## Risks

**Same-day repeats are impossible until M4, and it will feel wrong.** Rating a card Again sends
it days out, which is exactly the gap ADR-0010 exists to close. Expect the milestone to feel
incomplete when dogfooded; do not fix it here by inventing scheduling policy inside
`select_next`, because that is the throwaway code M3 deletes.

**`days_elapsed` for a card's first Review needs a defined value.** `next_states` takes
`Option<MemoryState>` and returns the new-card states when it is `None`; make sure the
first-Review path passes `None` rather than a zeroed `MemoryState`, since those are different
inputs to the crate.
