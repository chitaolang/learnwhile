# M2 manual test plan — Cards and Reviews

A by-hand checklist for the [M2 milestone](../../milestones/M2-cards-and-reviews.md): seed a deck,
do real Reviews during a wait, and confirm every rating persists and reschedules the card by FSRS.

The automated suite (`cargo test`) already covers all of this. This plan is for dogfooding against
the real binary, on a real terminal, the way a developer actually uses it.

## Setup

Do this once, in **every terminal** you use. The plan uses an isolated data directory so your real
deck is never touched, and starts with a build so you never test a stale binary.

```sh
cd /Users/timlin/learnwhile
cargo build --release
alias lw="$PWD/target/release/learnwhile"     # define in each terminal
export XDG_DATA_HOME=/tmp/lw-test              # isolate the deck; reset with: rm -rf /tmp/lw-test
```

You will use two terminals:

- **Terminal A** runs the host (a full-screen TUI).
- **Terminal B** runs `seed`, the hook, and inspection commands: `lw config` / `lw cards` for config and deck, and `sqlite3` (the `db` alias) for `review_history`, which has no subcommand.

Both need the alias and `XDG_DATA_HOME`. The socket is shared automatically at
`/tmp/learnwhile.sock` (your `XDG_RUNTIME_DIR` is unset). The database is at
`/tmp/lw-test/learnwhile/learnwhile.db`. A shortcut for the checks below:

```sh
alias db='sqlite3 -header -column /tmp/lw-test/learnwhile/learnwhile.db'
```

- [ ] The binary is fresh: `strings "$PWD/target/release/learnwhile" | grep -c trigger_expiry_seconds` prints a non-zero number.
- [ ] Make a seed file:

  ```sh
  printf 'What does FSRS stand for?\tFree Spaced Repetition Scheduler\nCapital of France\tParis\nRust ownership rule\tEach value has exactly one owner\n' > /tmp/deck.tsv
  ```

## 1. Seeding a deck is idempotent

- [ ] `lw seed /tmp/deck.tsv` prints `3 added, 0 skipped`.
- [ ] `lw seed /tmp/deck.tsv` again prints `0 added, 3 skipped`.
- [ ] Append a line and reseed adds only the new one:

  ```sh
  printf 'Borrow checker\tEnforces ownership at compile time\n' >> /tmp/deck.tsv
  lw seed /tmp/deck.tsv        # 1 added, 3 skipped
  ```
- [ ] `lw cards` lists 4 rows, each `state` = `new`, `due` shown as `-`, `reps` = `0`.

## 2. Seed is TSV-only and tolerates junk

- [ ] A file with blank lines and rows missing a side skips them without aborting:

  ```sh
  printf 'no tab on this line\n\n\tempty front\ngood\tcard\n' > /tmp/junk.tsv
  lw seed /tmp/junk.tsv        # 1 added (only "good\tcard")
  ```

## 3. Database and migration

- [ ] `db ".tables"` lists `cards config decks review_history`.
- [ ] `lw config` shows `trigger_expiry_seconds = 1800`, `desired_retention = 0.9`, `new_cards_per_day = 20`.

## 4. Idle vs Waiting, and the question side

- [ ] Terminal A: `lw host` shows `Not waiting` with footer `q quit`.
- [ ] Terminal B: `printf '{"session_id":"s1"}' | lw hook --open`.
- [ ] Terminal A flips to `Waiting`, shows the first card's **front** (`What does FSRS stand for?`), footer `space reveal    q quit`.
- [ ] The **answer is not on screen**: you cannot see `Free Spaced Repetition Scheduler` yet. (This is the premise-protecting check.)

## 5. Reveal and the rating keys

- [ ] Terminal A: press **space**. The back appears, footer becomes `1 Again   2 Hard   3 Good   4 Easy    q quit`.
- [ ] Press **3** (Good). The pane immediately advances to the next card's front (`Capital of France`).

## 6. Persistence is instant

Without quitting, in Terminal B:

- [ ] `db "SELECT card_id, rating, stability_after, difficulty_after, elapsed_days, scheduled_days FROM review_history;"` shows one row, `rating=3`, real numbers, `elapsed_days=0`.
- [ ] `lw cards` shows card `1` now `state` = `review`, `reps` = `1`, and a real future `due` date.

## 7. All four ratings, including Again

- [ ] Rate the remaining cards, one per rating, watching the pane advance each time: `space` then `1` (Again), `space` then `2` (Hard), `space` then `4` (Easy).
- [ ] `db "SELECT card_id, rating FROM review_history ORDER BY id;"` shows ratings `3, 1, 2, 4`.
- [ ] The Again card recorded a lapse: `lw cards` shows one card with `lapses` = `1`. An Again still counts as a completed Review. (Known M2 limitation: it is scheduled days out and will not return today.)

## 8. Deck exhaustion falls back to idle

- [ ] After all cards are reviewed, open a fresh trigger: `printf '{"session_id":"s2"}' | lw hook --open`.
- [ ] Terminal A stays on the idle pane, because `select_next` only picks unreviewed cards. This is correct, not a bug (real due-vs-new selection is M3).

## 9. A rating survives a restart

- [ ] Terminal A: press **q** to quit, then `lw host` again.
- [ ] `db "SELECT COUNT(*) FROM review_history;"` is unchanged. A crash or restart cannot lose a rating.

## 10. Config-driven expiry drains a lost close

- [ ] Quit the host, then shrink the expiry: `lw config set trigger_expiry_seconds 5`.
- [ ] Start `lw host`, open a trigger and never close it: `printf '{"session_id":"lost"}' | lw hook --open`.
- [ ] The card clears on its own within about 30 seconds (the sweep runs every 30s), with no close sent.
- [ ] Reset: `lw config set trigger_expiry_seconds 1800`.

## 11. Fail-open still holds

- [ ] Terminal A: quit the host.
- [ ] Terminal B: `printf '{"session_id":"x"}' | lw hook --open; echo "exit=$?"` prints `exit=0`. With no host running, the hook gives up instantly and never stalls the agent.

## Reset

Start over from an empty deck at any time:

```sh
rm -rf /tmp/lw-test
```

## Gotchas

- **Always run the binary you just built.** `cargo build` refreshes `target/debug/`, `cargo build --release` refreshes `target/release/`. Running an old one tests old code.
- **Config is read once, at host startup.** Change a `config` value while the host is stopped, then start it.
- **The sweep runs every 30 seconds.** Even a 5-second expiry does not clear a card until the next tick, so give it up to ~30 seconds.
