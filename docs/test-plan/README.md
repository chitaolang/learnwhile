# Manual test plan

One by-hand dogfooding checklist for LearnWhile, against the real binary on a real terminal. It
covers seeding and Reviews, honest scheduling, session continuity, furigana display, and the Prompt
Gate. The automated suite (`cargo test`) already covers the logic of all of it; this plan exists for
what the harness cannot show: a real TUI, real terminal CJK width, and the hook's real exit code.

Sections map to the milestones in [`../milestones/`](../milestones/README.md). Do the Setup once per
terminal, then work top to bottom, or jump to a feature. Sections 6 onward each reset and reseed, so
they stand alone.

## Setup

Do this once in **every terminal** you use. It isolates data, state, and the socket under
`/tmp/lw-test` so nothing touches your real deck, and builds first so you never test a stale binary.

```sh
cd /Users/timlin/learnwhile
cargo build --release
alias lw="$PWD/target/release/learnwhile"
export XDG_DATA_HOME=/tmp/lw-test/data
export XDG_STATE_HOME=/tmp/lw-test/state
export XDG_RUNTIME_DIR=/tmp/lw-test/run
mkdir -p "$XDG_RUNTIME_DIR"
SOCK="$XDG_RUNTIME_DIR/learnwhile.sock"
alias db='sqlite3 -header -column "$XDG_DATA_HOME"/learnwhile/learnwhile.db'
alias showlog='cat "$XDG_STATE_HOME"/learnwhile/host.log.* 2>/dev/null'
lwopen()  { printf '{"session_id":"s"}' | lw hook --open;  }         # hand off to the agent
lwclose() { printf '{"session_id":"s"}' | lw hook --close; }         # the agent returns
lwgate()  { printf '{"session_id":"s"}' | lw hook --gate --open; echo " (exit=$?)"; }  # gated handoff
```

Two terminals throughout:

- **Terminal A** runs the host (`lw host`, a full-screen TUI). Use at least ~50 columns so the
  furigana wrap check has room.
- **Terminal B** runs `seed`, the hook helpers, and `db` / `showlog` inspection.

Both need the whole block, especially `XDG_RUNTIME_DIR`, or B's probes hit a different socket than
A's host.

- [ ] The binary is fresh: `strings "$PWD/target/release/learnwhile" | grep -c trigger_expiry_seconds` prints a non-zero number.
- [ ] Seed a starter deck for sections 1 through 5:

  ```sh
  printf 'What does FSRS stand for?\tFree Spaced Repetition Scheduler\nCapital of France\tParis\nRust ownership rule\tEach value has exactly one owner\n' > /tmp/deck.tsv
  ```

## 1. Seeding is idempotent and TSV-only

- [ ] `lw seed /tmp/deck.tsv` prints `3 added, 0 skipped`; running it again prints `0 added, 3 skipped`.
- [ ] Append a line and reseed adds only the new one:

  ```sh
  printf 'Borrow checker\tEnforces ownership at compile time\n' >> /tmp/deck.tsv
  lw seed /tmp/deck.tsv        # 1 added, 3 skipped
  ```
- [ ] Junk is skipped without aborting: `printf 'no tab here\n\n\tempty front\ngood\tcard\n' > /tmp/junk.tsv && lw seed /tmp/junk.tsv` adds only `good\tcard` (`1 added`).
- [ ] `lw cards` lists the seeded rows, each `state` = `new`, `due` shown as `-`, `reps` = `0`, with the **raw** authored front text (the admin listing is deliberately un-rendered).

## 2. Database and migration

- [ ] `db ".tables"` lists exactly `cards config decks review_history`.
- [ ] `lw config` shows `trigger_expiry_seconds = 1800`, `desired_retention = 0.9`, `new_cards_per_day = 20`.
- [ ] `db ".schema cards"` has no `reading` (or other furigana) column: readings ride inside the existing `front`/`back` text (ADR-0012), no migration.

## 3. Idle vs Waiting, and the hidden question side

- [ ] Terminal A: `lw host` shows `Not waiting` with footer `q quit`.
- [ ] Terminal B: `lwopen`. Terminal A flips to `Waiting`, shows the first card's **front**, footer `space reveal    q quit`.
- [ ] The **answer is not on screen** yet. (The premise-protecting check.)

## 4. Reveal, rating, and instant persistence

- [ ] Terminal A: press **space**. The back appears, footer becomes `1 Again   2 Hard   3 Good   4 Easy    q quit`.
- [ ] Press **3** (Good). The pane immediately advances to the next card's front.
- [ ] Without quitting, Terminal B: `db "SELECT card_id, rating, elapsed_days FROM review_history;"` shows one row, `rating=3`, `elapsed_days=0`.
- [ ] `lw cards` shows that card now `state` = `review`, `reps` = `1`, with a real future `due` date.

## 5. All four ratings, including Again

- [ ] Rate the remaining cards one per rating, watching the pane advance: `space` then `1` (Again), `space` then `2` (Hard), `space` then `4` (Easy).
- [ ] `db "SELECT rating FROM review_history ORDER BY id;"` shows `3, 1, 2, 4`.
- [ ] The Again card recorded a lapse: `lw cards` shows one card with `lapses` = `1`. An Again is a completed Review, rescheduled by FSRS, not a skipped one.
- [ ] A rating survives a restart: press **q**, `lw host` again, and `db "SELECT COUNT(*) FROM review_history;"` is unchanged.

## 6. Honest scheduling: capped, and never pulled forward

Reset and seed five numbered cards, and lower the cap so it is exhaustible by hand (config is read
at startup, so set it while the host is stopped):

```sh
rm -rf /tmp/lw-test && mkdir -p /tmp/lw-test/run
printf 'front1\tback1\nfront2\tback2\nfront3\tback3\nfront4\tback4\nfront5\tback5\n' > /tmp/deck.tsv
lw seed /tmp/deck.tsv
lw config set new_cards_per_day 2
```

- [ ] Start `lw host` **without** a Trigger. The idle pane shows `Not waiting`, `Due now: 0    New remaining: 2`, and `Next due: nothing scheduled`. `New remaining` equals the cap because nothing is introduced yet.
- [ ] `lwopen` surfaces `front1`. Reveal and rate it Good; the pane advances to `front2` within the same wait. Reveal and rate `front2` Good. The cap of 2 is now spent: the pane shows the idle state, `New remaining: 0`, and does **not** show `front3`. The deck was not dumped on you.
- [ ] `db "SELECT COUNT(*) FROM review_history WHERE stability_before IS NULL"` returns `2` (two introductions).
- [ ] `lwclose` then `lwopen`. The pane stays idle: neither `front1` (rated, due days out) nor `front3` (new, over cap) appears. **This is the ADR-0002 guarantee**: nothing is pulled forward ahead of its due date.
- [ ] The cap survives a restart. Quit, `lw host`, `lwopen`: still idle, `New remaining: 0`. The count is derived from `review_history`, not a counter.
- [ ] Raising the cap lets more in the same day. Quit, `lw config set new_cards_per_day 4`, start, `lwopen`: `front3` surfaces again. `lw cards` shows the early cards as `review`, the rest still `new`.
- [ ] The idle pane distinguishes states: **not** Waiting reads `Not waiting`; Waiting with nothing to review reads `Waiting` with `Due now: 0`, `New remaining: 0`, so the counts explain the empty pane rather than a blank frame.
- [ ] Empty deck is sensible: quit, `rm -rf /tmp/lw-test && mkdir -p /tmp/lw-test/run`, `lw host`, `lwopen`. The pane shows `Due now: 0    New remaining: 0` and `Next due: nothing scheduled`. No panic, no blank frame.

**Covered only by `cargo test`** (they need the clock advanced across days, which a human cannot do):
a due card returns once due and beats a new one; the cap rolls over past local midnight; the
local-timezone day boundary is off-by-one-safe. Run `cargo test scheduling` and `cargo test --lib learning`.

## 7. Session continuity: resume, and re-attempt a failure

A Session is the host process lifetime (ADR-0011): quitting ends it, so a restart is a fresh Session
with an empty lapse queue. Reset and seed two cards so the lapsed card is distinguishable from a new
one:

```sh
rm -rf /tmp/lw-test && mkdir -p /tmp/lw-test/run
printf 'front1\tback1\nfront2\tback2\n' > /tmp/deck.tsv
lw seed /tmp/deck.tsv
```

- [ ] **Mid-review resumes.** `lw host`, `lwopen` (shows `front1`), press **space** to reveal `back1`, do **not** rate. `lwclose` (agent returns); the pane clears. `lwopen` again: the **same** card is back, **still showing `back1`** and the rating footer. You are not asked to re-recall.
- [ ] **A revealed card survives its Trigger expiring.** Quit, `lw config set trigger_expiry_seconds 5`, `lw host`. `lwopen`, reveal, do not rate. Wait ~30s (the sweep runs every 30s) without a `lwclose`: the Trigger expires and the pane clears. `lwopen`: the card resumes, still revealed. Reset: quit, `lw config set trigger_expiry_seconds 1800`, `lw host`.
- [ ] **Ignoring carries no penalty.** `lwopen` (`front1`), do not reveal or rate. `lwclose`/`lwopen` a few cycles: the card is unchanged each time and `db "SELECT COUNT(*) FROM review_history"` is still `0`.
- [ ] **A failed card returns for a re-attempt.** `lwopen`, **space**, **1** (Again). The pane immediately shows `front1` **again as a question** (answer hidden). It is `front1`, not `front2`: the lapse queue re-offers it ahead of the new card.
- [ ] **A re-attempt converges once passed.** With `front1` up as a question, **space** then **3** (Good). `front2` (the new card) now appears and `front1` does **not** loop back.
- [ ] **The lapse queue dies on restart.** `lwopen`, reveal `front1`... rate **1** (re-offered). Quit (**q**), `lw host` again (new Session), `lwopen`: `front2` appears, **not** `front1`. The queue did not survive; the failed card sits at its persisted future due date (`lw cards` shows `front1` as `review`, `lapses` = `1`, `due` a day or more ahead).

## 8. Install hardening: single instance, recovery, logging, signals

Reset and seed one card so the host has content:

```sh
rm -rf /tmp/lw-test && mkdir -p /tmp/lw-test/run
printf 'front1\tback1\n' > /tmp/deck.tsv && lw seed /tmp/deck.tsv
```

- [ ] **The log is created where the README says.** `lw host` in A; in B `showlog` prints a line like `INFO learnwhile host starting`, `ls "$XDG_DATA_HOME"/learnwhile/` shows `learnwhile.db`, and `ls "$XDG_STATE_HOME"/learnwhile/` shows a `host.log.<date>` file.
- [ ] **A second host refuses.** With A running, `lw host` in B refuses immediately with a message naming the socket, exits non-zero (`echo $?`), and A is undisturbed.
- [ ] **A killed host recovers from its stale socket.** `pkill -9 -f 'learnwhile host'` in B (SIGKILL leaves A's TUI as-is). `ls -l "$SOCK"` still shows the socket. `lw host` in A starts cleanly with no manual cleanup: the stale socket is detected by a failed connect probe and unlinked.
- [ ] **Discarded frames are logged.** With the host running, `printf 'not-a-frame\n' | nc -U "$SOCK"` (Ctrl-C if it hangs; the frame was already read). A is unaffected; after a second, `showlog` shows `WARN discarded trigger frame reason=Unparseable`. An unknown version logs a different reason: `printf '{"v":99,"type":"trigger_open","adapter":"x","session":"s","at":"2026-01-01T00:00:00Z"}\n' | nc -U "$SOCK"` then `showlog` shows `reason=UnknownVersion(99)`. A valid frame still works afterward: `lwopen` surfaces `front1`, so the accept loop survived.
- [ ] **The terminal is restored on a signal.** With `lw host` running in A, `pkill -TERM -f 'learnwhile host'` from B: A's pane closes and the shell returns cleanly (`echo restored` echoes normally, not stuck in the alternate screen). Repeat with `pkill -INT`. Pressing **Ctrl-C** directly in A also exits cleanly (crossterm hands it to the app as a key). SIGKILL is the one signal that cannot restore, which is the stale-socket case above.
- [ ] **Log growth is bounded.** The filename carries a date suffix (`host.log.2026-...`), which is daily rotation: a long-lived host writes a new file each day rather than one unbounded file.

## 9. Furigana display

Reset and seed a Japanese deck. A leading delimiter space is trimmed by `seed`, so scoping spaces go
*inside* the field (`「 勉強[べんきょう]」`), never at the very start:

```sh
rm -rf /tmp/lw-test && mkdir -p /tmp/lw-test/run
printf '勉強[べんきょう]\tstudy\n食[た]べる\tto eat\n日本[にほん]語[ご]\tJapanese language\n「 勉強[べんきょう]」は 英語[えいご]で 何[なん]と 言[い]いますか？\tHow do you say "study" in English?\ncost[ is high\tno reading\n配列［0］とは？\tarray index zero\nWhat does FSRS stand for?\tFree Spaced Repetition Scheduler\n' > /tmp/jp-deck.tsv
lw seed /tmp/jp-deck.tsv        # 7 added, 0 skipped
```

Run `lw host` and `lwopen`, then reveal (**space**) and advance (**3**) through the deck:

- [ ] **Hidden on the question side.** The first card's front shows the base kanji only, **`勉強`**; `べんきょう` is nowhere on screen (ADR-0013).
- [ ] **Reveal stacks the reading over its kanji**, centered, back below:

  ```
  べんきょう
     勉強
  study
  ```
- [ ] **Okurigana stays plain.** `食べる` shows no reading on the question; on reveal `た` sits over `食` only, `べる` has blank space above it.
- [ ] **Adjacent words each keep their reading.** `日本語` reveals with `にほん` over `日本` and `ご` over `語`.
- [ ] **A long sentence wraps by unit.** The question side shows base-only `「勉強」は英語で何と言いますか？` (delimiter spaces gone). On reveal it wraps to fit the pane and every reading stays directly above its kanji, none stranded. Narrow the pane and reopen: the wrap point moves but no reading/kanji pair ever splits.
- [ ] **A malformed annotation is literal.** `cost[ is high` shows exactly, one plain line; on reveal the front is unchanged, no panic, pane still responsive.
- [ ] **Full-width brackets are literal.** `配列［0］とは？` shows the full-width `［ ］` (distinct codepoints from ASCII, never parsed as a reading).
- [ ] **An unannotated card is unchanged.** `What does FSRS stand for?` renders as a plain single line with no reading line, front then blank then back, exactly like a pre-furigana card.

## 10. Prompt Gate

Reset and seed a small deck:

```sh
rm -rf /tmp/lw-test && mkdir -p /tmp/lw-test/run
printf 'Capital of France\tParis\nRust ownership rule\tEach value has exactly one owner\nBorrow checker\tEnforces ownership at compile time\n' > /tmp/deck.tsv
lw seed /tmp/deck.tsv
```

`lwgate` performs the same round-trip Claude Code's gated `UserPromptSubmit` hook does, and prints
the verdict then `(exit=N)`: an allow shows just ` (exit=0)`, a block shows
`{"decision":"block","reason":"Finish one review to continue."} (exit=0)`.

- [ ] **Gate off is v1.** `lw host`, then `lwopen` surfaces a card and `lwclose` clears it. No prompt is blocked and the idle pane shows no card.
- [ ] **First gated prompt is allowed and surfaces a card.** `lwgate` prints ` (exit=0)`; Terminal A flips to `Waiting` with the first card. The gate opened the Trigger itself on allow.
- [ ] **An owed Review holds the next prompt.** Do not review. `lwclose` (agent returns): Terminal A keeps the card with heading **`Review to continue`** even while idle. `lwgate` again prints the block JSON and Terminal A still shows the owed card, no new wait opened.
- [ ] **Pay from the idle pane.** With the owed card up, **space** then **3**: Terminal A returns to `Not waiting`. `lwgate` now prints ` (exit=0)` and surfaces the next card.
- [ ] **Reviewing during the wait also clears the debt.** With a card up (Waiting), **space** then **3**, then `lwclose`, then `lwgate`: allowed.
- [ ] **Allowed when nothing is reviewable.** Finish the deck (rate any remaining), until `lwgate` leaves Terminal A idle with `Due now: 0`, `New remaining: 0`. With no card to surface, no debt is incurred, so the gate cannot block.
- [ ] **Fail-open: no host never blocks.** Press **q** to quit. `lwgate` returns immediately with ` (exit=0)` and no block output.
- [ ] **Debt does not survive a restart.** `lw host`, `lwgate` (allowed), do not review, `lwclose`. Quit (**q**), `lw host` again, `lwgate`: allowed. The debt reset with the new host process.
- [ ] **The verdict is a JSON frame on the wire** (ADR-0007). Get into the owed state (`lwgate`, `lwclose`, so Terminal A shows `Review to continue`), then dial the socket by hand:

  ```sh
  printf '{"v":1,"type":"gate_query","adapter":"claude-code","session":"probe","at":"2026-01-01T00:00:00Z"}\n' | nc -U "$SOCK"
  ```

  The reply line is JSON, `{"v":1,"type":"gate_verdict","verdict":"block"}`, not a bare `block` token. Press **Ctrl-C** once you see it. (This host-to-hook reply is distinct from the hook-to-Claude `{"decision":"block",...}` output above.)

## Reset

Start over from an empty deck at any time:

```sh
rm -rf /tmp/lw-test && mkdir -p /tmp/lw-test/run
```

## Gotchas

- **Always run the binary you just built.** `cargo build --release` refreshes `target/release/`; running an old one tests old code.
- **Config is read once, at host startup.** Change a `config` value while the host is stopped, then start it. This applies to the cap and the expiry.
- **The sweep runs every 30 seconds.** Even a 5-second expiry does not clear a card until the next tick, so allow up to ~30s.
- **A restart is a fresh Session.** The lapse queue and the gate's review debt are in-memory only and reset after `q` and a new `lw host`. That is by design (ADR-0010, ADR-0011), not data loss. An Again card is re-offered within the Session by the queue, and separately rescheduled by FSRS to a future due date.
- **The gate needs a live host, and the host is a TUI.** Run it in a real terminal; a non-tty host exits at raw-mode setup and cannot answer a gate query. The gate is only ever as present as the host: dropping `--gate` or quitting always lets prompts through.
- **The owed card shows while idle only after a gate query.** A session that only runs `lwopen`/`lwclose` never shows a card on the idle pane; v1 behavior is untouched.
- **Furigana alignment depends on your terminal's CJK width.** The layout assumes kanji and kana are two columns wide; a terminal or font that renders them otherwise drifts the reading off its kanji. That is a terminal setting, not a LearnWhile bug. The delimiter space scopes the base (`お茶[ちゃ]` puts `ちゃ` over all of `お茶`; write `お 茶[ちゃ]` to scope to `茶`), and is consumed, not shown.
- **`nc -U` is netcat's unix-socket mode** (ships with macOS) and holds the connection open, printing the reply then waiting; `socat - UNIX-CONNECT:"$SOCK"` is the equivalent if your `nc` lacks `-U`.
- **The log flushes asynchronously**, so a discarded-frame line can take a second to appear. That buffering is what keeps logging off the hot path.
```
