# M3 manual test plan — Honest scheduling

A by-hand checklist for the [M3 milestone](../../milestones/README.md#m3-honest-scheduling): a developer
can trust the pane to show a genuinely due card, else a new card within the day's cap, else an idle
state that explains itself, and never a card pulled forward ahead of its due date.

The automated suite (`cargo test`) already covers all of this. This plan is for dogfooding against
the real binary. It builds on the [M2 plan](../M2/README.md), so read that first if you have not.

## Setup

Do this once, in **every terminal** you use. Isolated data directory, fresh build.

```sh
cd /Users/timlin/learnwhile
cargo build --release
alias lw="$PWD/target/release/learnwhile"     # define in each terminal
export XDG_DATA_HOME=/tmp/lw-m3                # isolate the deck; reset with: rm -rf /tmp/lw-m3
alias db='sqlite3 -header -column /tmp/lw-m3/learnwhile/learnwhile.db'
```

Two terminals: **A** runs the host (`lw host`, a TUI); **B** runs `seed`, the hook, and `db`
checks. Both need the alias and `XDG_DATA_HOME`. Socket is shared at `/tmp/learnwhile.sock`.

Two facts about M3 that shape every test below:

- **The daily cap is 20 by default**, which is too many to exhaust by hand. Lower it to make the
  cap testable: `lw config set new_cards_per_day 2`. The host reads it
  at startup, so set it while the host is stopped.
- **"Due" and the day boundary are time-based.** By hand you cannot fast-forward days, so the tests
  that need a due card or a cap rollover are marked as **needs time travel** and are only fully
  checked by `cargo test` (which injects a clock). You can still verify the same-day behaviour by
  hand.

Seed a small deck (five cards):

```sh
printf 'front1\tback1\nfront2\tback2\nfront3\tback3\nfront4\tback4\nfront5\tback5\n' > /tmp/deck.tsv
lw seed /tmp/deck.tsv
```

## 1. The idle pane is real, not a placeholder

- [ ] Start `lw host` in Terminal A **without** opening a Trigger. The pane shows `Not waiting` and a line like `Due now: 0    New remaining: 2` plus `Next due: nothing scheduled`.
- [ ] The `New remaining` figure equals your cap (you set it to 2), because no new cards have been introduced yet.

## 2. New cards are introduced one at a time, up to the cap

Cap is 2. In Terminal B, drive Triggers; watch Terminal A.

- [ ] `printf '{"session_id":"s"}' | lw hook --open` surfaces the first new card (`front1`).
- [ ] Press space, then `3` (Good). The pane advances to the **second** new card (`front2`) within the same wait.
- [ ] Reveal and rate `front2` Good. Now the cap of 2 is spent. The pane shows the idle state, `New remaining: 0`, and does **not** show `front3`.
- [ ] Confirm the count in the database: `db "SELECT COUNT(*) FROM review_history WHERE stability_before IS NULL"` returns `2`.

This is the cap holding within a day: the deck was not dumped on you.

## 3. A rated card does not come back the same day

- [ ] With the two cards reviewed and the cap spent, close and reopen the Trigger: `printf '{"session_id":"s"}' | lw hook --close` then `... --open`.
- [ ] The pane stays idle. Neither `front1` (rated, due days out) nor `front3` (new, over the cap) appears. **This is the ADR-0002 guarantee**: nothing is pulled forward ahead of its due date.
- [ ] Quit the host with `q`.

## 4. The cap survives a restart

- [ ] Restart the host: `lw host` again (same `XDG_DATA_HOME`, so the same database).
- [ ] Open a Trigger: `printf '{"session_id":"s"}' | lw hook --open`. The pane is idle and `New remaining: 0`. The two introductions made before the restart still count, because the cap is derived from `review_history`, not a counter.

## 5. Raising the cap lets more new cards in (same day)

- [ ] Quit the host. Raise the cap: `lw config set new_cards_per_day 4`.
- [ ] Start the host, open a Trigger. A new card (`front3`) surfaces again, because the cap now allows two more today.
- [ ] `lw cards` shows the first cards as `review` and the later ones still `new`.

## 6. The idle pane distinguishes "nothing due" from "not waiting"

- [ ] While **not** Waiting (no open Trigger), the pane header reads `Not waiting`.
- [ ] While Waiting but with nothing to review (cap spent, nothing due), the header reads `Waiting` and the counts explain why nothing surfaced (`Due now: 0`, `New remaining: 0`). The two states are told apart by the header plus the counts, not by a blank pane.

## 7. Empty deck renders sensibly

- [ ] Reset to an empty deck: quit the host, `rm -rf /tmp/lw-m3`, then start `lw host` (this recreates the database with no cards).
- [ ] Open a Trigger. The pane shows `Due now: 0    New remaining: 0` and `Next due: nothing scheduled`. No blank frame, no panic.

## Needs time travel (verified by `cargo test`, not by hand)

These depend on advancing the clock across days, which the injected test clock does and a human
cannot. They are listed so you know what is covered and why it is not in the manual steps.

- [ ] **A due card returns once due, and beats a new one.** After a card's interval elapses it is surfaced again, ahead of introducing new cards. Covered by `tests/scheduling.rs::a_reviewed_card_returns_only_once_it_is_due` and the `learning.rs` selection unit tests.
- [ ] **The cap rolls over past local midnight.** A new day resets the introduction count. Covered by `tests/scheduling.rs::the_daily_cap_rolls_over_after_local_midnight`.
- [ ] **The local-timezone day boundary is off-by-one-safe.** Covered by the `local_day_bounds` unit tests with fixed UTC offsets near midnight.

To run them:

```sh
cargo test scheduling
cargo test --lib learning
```

## Reset

```sh
rm -rf /tmp/lw-m3
```

## Gotchas

- **Run the binary you just built**, and set the cap while the host is **stopped** (config is read once, at startup).
- **A rated card will not reappear today even if you rated it Again.** That is a known v1 limitation, not an M3 bug: same-day return arrives in M4 under ADR-0010.
- The default cap of 20 is a deliberate guess, left configurable and untuned until `review_history` holds real data.
