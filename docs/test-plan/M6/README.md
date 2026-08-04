# M6 manual test plan — Furigana display

A by-hand checklist for the [M6 milestone](../../milestones/README.md#m6-furigana-display): seed a Japanese
deck with inline readings and confirm the reading is hidden on the question side and stacked over
its kanji on reveal, that unannotated cards render exactly as before, and that malformed notation
degrades to literal text without crashing.

The automated suite (`cargo test`) already covers all of this. This plan is for dogfooding against
the real binary, on a real terminal, where font and terminal CJK-width rendering are in play in a
way the `TestBackend` cannot show.

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

- **Terminal A** runs the host (a full-screen TUI). Use a window at least ~50 columns wide so the
  wrapping check has room to be interesting.
- **Terminal B** runs `seed`, the hook, and inspection commands: `lw cards` for the deck, and
  `sqlite3` (the `db` alias) for a look at the schema.

Both need the alias and `XDG_DATA_HOME`. The socket is shared automatically at
`/tmp/learnwhile.sock` (your `XDG_RUNTIME_DIR` is unset). The database is at
`/tmp/lw-test/learnwhile/learnwhile.db`. Shortcuts for below:

```sh
alias db='sqlite3 -header -column /tmp/lw-test/learnwhile/learnwhile.db'
lwopen()  { printf '{"session_id":"s"}' | lw hook --open;  }
lwclose() { printf '{"session_id":"s"}' | lw hook --close; }
```

- [ ] The binary is fresh: `strings "$PWD/target/release/learnwhile" | grep -c trigger_expiry_seconds` prints a non-zero number.
- [ ] Make the seed file. A leading delimiter space is trimmed by `seed`, so scoping spaces go
  *inside* the field (e.g. `「 勉強[べんきょう]」`), never at the very start:

  ```sh
  printf '勉強[べんきょう]\tstudy\n食[た]べる\tto eat\n日本[にほん]語[ご]\tJapanese language\n「 勉強[べんきょう]」は 英語[えいご]で 何[なん]と 言[い]いますか？\tHow do you say "study" in English?\ncost[ is high\tno reading\n配列［0］とは？\tarray index zero\nWhat does FSRS stand for?\tFree Spaced Repetition Scheduler\n' > /tmp/jp-deck.tsv
  ```

## 1. Seeding and the admin surface

- [ ] `lw seed /tmp/jp-deck.tsv` prints `7 added, 0 skipped`.
- [ ] `lw cards` lists 7 rows, all `state` = `new`. The `front` column shows the **raw** authored
  text, readings and all: `勉強[べんきょう]`, `食[た]べる`, and the long sentence with its internal
  spaces intact. (The admin listing is deliberately un-rendered, spec §Admin surface.)

## 2. The card model and schema are untouched

- [ ] `db ".tables"` still lists exactly `cards config decks review_history`. No new table.
- [ ] `db ".schema cards"` has no `reading` (or other new) column. Furigana added no migration:
  the annotation rides inside the existing `front`/`back` text (ADR-0012).

## 3. The reading is hidden on the question side

- [ ] Terminal A: `lw host` shows `Not waiting`.
- [ ] Terminal B: `lwopen`. Terminal A flips to `Waiting` and the first card comes up.
- [ ] The front shows the base kanji only: **`勉強`**.
- [ ] The reading **`べんきょう` is nowhere on screen.** This is the premise-protecting check: a card
  testing a reading must not hand over the reading (ADR-0013).

## 4. Reveal stacks the reading over its kanji

- [ ] Terminal A: press **space**. The pane now shows the reading on the line directly above the
  kanji, centered over it, with the back below:

  ```
  べんきょう
     勉強
  study
  ```
- [ ] Advance with **3** (Good).

## 5. Okurigana keeps trailing kana plain

- [ ] The next card's question side shows **`食べる`** (no reading visible).
- [ ] Press **space**. The reading `た` sits over `食` only; `べる` has blank space above it; the
  back `to eat` shows below:

  ```
  た
  食べる
  to eat
  ```
- [ ] Advance with **3**.

## 6. Adjacent words each keep their own reading

- [ ] Question side shows **`日本語`**.
- [ ] Press **space**. `にほん` sits over `日本` and `ご` over `語`, each reading above its own
  kanji, back `Japanese language` below.
- [ ] Advance with **3**.

## 7. A long sentence wraps by unit, reading never leaving its kanji

- [ ] Question side shows the base-only sentence with the delimiter spaces gone:
  **`「勉強」は英語で何と言いますか？`** (no readings, no stray spaces).
- [ ] Press **space**. The sentence renders as reading-over-base and **wraps** to fit the pane. On
  every wrapped line, each reading stays directly above the kanji it belongs to (`べんきょう` over
  `勉強`, `えいご` over `英語`, `なん` over `何`, `い` over `言`); no reading is ever stranded on a
  line away from its kanji. The back `How do you say "study" in English?` shows below.
- [ ] Narrow Terminal A by a few columns and reopen (`q`, `lw host`, `lwopen`, rate up to this card
  again, or just resize before revealing): the wrap point moves but no reading/kanji pair ever
  splits.
- [ ] Advance with **3**.

## 8. A malformed annotation is left literal, and the host survives it

- [ ] Question side shows **`cost[ is high`** exactly, as a plain single line. An unclosed `[` is
  not markup.
- [ ] Press **space**. Front stays `cost[ is high`, back shows `no reading`. No panic, no garbled
  pane, and Terminal A is still responsive.
- [ ] Advance with **3**.

## 9. Full-width brackets are literal, not markup

- [ ] Question side shows **`配列［0］とは？`** with the full-width `［ ］` visible. These are
  distinct codepoints from ASCII `[ ]` and are never parsed as a reading (the documented escape
  hatch, spec §Notation).
- [ ] Press **space**, then **3** to advance.

## 10. An unannotated card renders exactly as before

- [ ] The last card is the English one. Its question side shows **`What does FSRS stand for?`** on a
  single line, with **no** reading line above it.
- [ ] Press **space**. Front and back render just like an M2 card: front line, blank line, back
  (`Free Spaced Repetition Scheduler`), word-wrapped by ratatui. Adding furigana cost this card
  nothing (the passthrough path).
- [ ] Rate it (**3**). The deck is now exhausted; the pane returns to idle.

## 11. Fail-open and passivity still hold

- [ ] `lwclose` then `lwopen` a few times: the pane tracks Waiting/idle as before. Furigana is a
  render-only change and adds no new input or focus behavior (ADR-0001).

## Reset

Start over from an empty deck at any time:

```sh
rm -rf /tmp/lw-test
```

## Gotchas

- **Always run the binary you just built.** Running an old `target/release/learnwhile` tests old
  code.
- **The delimiter space scopes the base.** `お茶[ちゃ]` puts `ちゃ` over the whole `お茶`; to scope
  it to `茶` only, write `お 茶[ちゃ]`. This matches Anki exactly. The space is consumed, not shown.
- **A leading space is trimmed by `seed`.** Scoping spaces must sit inside the field, not at its
  start. `「 勉強[べんきょう]」` works; a field beginning with a space loses it.
- **Alignment depends on your terminal's CJK width.** The layout assumes kanji and kana are two
  columns wide. A terminal or font that renders them at a different width will drift the reading
  off its kanji. That is the terminal's setting, not a LearnWhile bug.
- **`lw cards` shows raw notation on purpose.** Seeing `勉強[べんきょう]` there is expected; it is
  what the author typed and what you debug against.
