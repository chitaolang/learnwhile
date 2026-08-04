# M4 manual test plan — Session continuity

A by-hand checklist for the [M4 milestone](../../milestones/README.md#m4-session-continuity): a review
survives the rhythm of real agent use. A card you were halfway through is still there on the next
wait, and a card you failed comes back for a second attempt before the sitting ends.

The automated suite (`cargo test`) already covers all of this. This plan is for dogfooding against
the real binary. It builds on the [M2](../M2/README.md) and [M3](../M3/README.md) plans.

## Setup

Do this once, in **every terminal** you use. Isolated data directory, fresh build.

```sh
cd /Users/timlin/learnwhile
cargo build --release
alias lw="$PWD/target/release/learnwhile"     # define in each terminal
export XDG_DATA_HOME=/tmp/lw-m4                # isolate the deck; reset with: rm -rf /tmp/lw-m4
alias db='sqlite3 -header -column /tmp/lw-m4/learnwhile/learnwhile.db'
```

Two terminals: **A** runs the host (`lw host`, a TUI); **B** runs `seed`, the hook, and `db`
checks. Both need the alias and `XDG_DATA_HOME`. Socket is shared at `/tmp/learnwhile.sock`.

Three facts about M4 that shape the tests:

- **A Session is the host process lifetime** ([ADR-0011](../../adr/0011-session-is-host-process-lifetime.md)). Quitting the host ends the Session, so a restart is a fresh Session with an empty lapse queue.
- **Rating Again now schedules the card at least a day out.** So a failed card does not come back the same day *by being due* — the in-memory lapse queue is the only thing that re-offers it this Session. That is the whole point of M4.
- **The re-offer is immediate.** After you rate a card Again while still Waiting, the host re-surfaces it right away as a fresh question (the answer hidden again). You do not have to wait for a new Trigger, though a new Trigger works too.

Seed two cards so you can tell "the lapsed card" apart from "a new card":

```sh
printf 'front1\tback1\nfront2\tback2\n' > /tmp/deck.tsv
lw seed /tmp/deck.tsv
```

Start `lw host` in Terminal A. In Terminal B, define two helpers (`lwopen`/`lwclose`, avoiding a
clash with the macOS `lwopen` command):

```sh
lwopen()  { printf '{"session_id":"s"}' | lw hook --open;  }
lwclose() { printf '{"session_id":"s"}' | lw hook --close; }
```

## 1. A mid-review resumes after the agent returns

- [ ] `lwopen` in Terminal B. Terminal A shows `front1`.
- [ ] Press space in Terminal A. The answer `back1` appears. **Do not rate it.**
- [ ] `lwclose` in Terminal B (the agent came back). The pane clears to the idle state.
- [ ] `lwopen` again. The **same** card is back, **still showing `back1`** and the rating footer. You are not asked to recall something you already revealed.

## 2. A revealed card survives its Trigger expiring

- [ ] Quit the host (`q`). Shrink the expiry: `lw config set trigger_expiry_seconds 5`. Start `lw host` again.
- [ ] `lwopen`, press space to reveal `back1`, and do not rate.
- [ ] Wait about 30 seconds without sending a `lwclose`. The Trigger expires and the pane clears on its own.
- [ ] `lwopen` again. The card resumes, still revealed. Expiry cleared the pane but did not discard the in-flight review.
- [ ] Reset the expiry: quit, `lw config set trigger_expiry_seconds 1800`, restart.

## 3. Ignoring a card carries no penalty

- [ ] `lwopen`. `front1` appears. Do not reveal or rate it.
- [ ] `lwclose` then `lwopen` a few times, letting the pane clear and come back each cycle.
- [ ] The card is still there each time, unchanged. Nothing reviewed itself: `db "SELECT COUNT(*) FROM review_history"` is still `0`.

## 4. A failed card returns for a re-attempt

- [ ] `lwopen`. `front1` appears. Press space, then `1` (Again).
- [ ] The pane immediately shows `front1` **again as a question** (the answer `back1` is hidden now). That is the re-attempt, offered by the lapse queue.
- [ ] It is `front1`, **not** `front2`: the lapsed card comes back ahead of the new card.

## 5. A re-attempt converges once you pass it

- [ ] Continuing from section 4, `front1` is up as a question. Press space, then `3` (Good).
- [ ] Now `front2` (the new card) appears. `front1` does **not** come back: a re-attempt rated Good leaves the queue instead of looping.

## 6. The lapse queue dies on restart

- [ ] Reset and reseed: quit the host, `rm -rf /tmp/lw-m4`, `lw seed /tmp/deck.tsv`, `lw host`.
- [ ] `lwopen`, reveal `front1`, rate `1` (Again). `front1` is re-offered (queue holds it).
- [ ] Quit the host with `q`, then start `lw host` again. This is a new Session.
- [ ] `lwopen`. `front2` (a new card) appears, **not** `front1`. The failed card is not re-offered, because the queue did not survive and the card is due tomorrow, not now.
- [ ] Confirm it sits at a future due date: `lw cards` shows `front1` as `review` with a `due` a day or more ahead.

## 7. Again is a completed Review that reschedules

- [ ] After rating a card Again in any section above, `db "SELECT card_id, rating FROM review_history ORDER BY id DESC LIMIT 1"` shows a row with `rating = 1`. A Lapse is a completed Review, not a skipped one.
- [ ] The card was rescheduled, not left null: `lw cards` shows the failed card as `review` with a real `due` and `lapses` = `1`.

## Reset

```sh
rm -rf /tmp/lw-m4
```

## Gotchas

- **Run the binary you just built.** The lapse queue and resume behavior are recent code.
- **A restart is a new Session.** The lapse queue is in memory only and is gone after `q` and a fresh `lw host` — that is by design ([ADR-0010](../../adr/0010-lapsed-cards-requeue-within-session.md)), not data loss.
- **After Again, the same card reappearing is the feature, not a stuck screen.** The tell is that the answer is hidden again: it is a fresh question, not the one you just rated.
