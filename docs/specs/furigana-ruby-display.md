# Furigana (ruby) display for Japanese cards

Vocabulary follows [`/CONTEXT.md`](../../CONTEXT.md). Decisions cited as ADR-NNNN from
[`docs/adr/`](../adr/). This spec is post-v1 and additive: it changes only how a card is
*drawn*, never how one is stored, scheduled, or selected.

## Problem Statement

A Japanese card's front is often a run of kanji whose reading the learner is trying to acquire.
On paper and in Anki that reading is set as *furigana*: small kana printed above the kanji
(ruby text). LearnWhile has no way to show a reading, so a Japanese deck author has two bad
choices: put the reading inline in the same field (which clutters the prompt and, on the
question side, gives the answer away), or leave it off and lose the single most useful piece of
information on the card.

Two constraints make this non-trivial in our surface:

1. A terminal is a fixed grid of full-size cells with one font size. True typographic ruby
   (half-height kana above the base glyph) is physically impossible. The only faithful
   rendering is a *stacked* one: the reading on its own full-height line directly above the
   base line, aligned column-for-column.
2. Kanji and kana are two display columns wide, and a reading is usually wider than the kanji
   it sits over. Alignment has to be computed in display columns, not characters or bytes, or
   the reading drifts off its kanji. This is the same wide-character accounting that the test
   harness's `buffer_text` was fixed to respect (commit `7b19b57`).

## Solution

Let a card author annotate readings inline using **Anki-compatible furigana notation**, and
render those annotations as a stacked reading-over-base layout. The reading is shown **only on
the answer side**. On the question side the annotated text renders as plain kanji, so a
"read this kanji" card stays an honest recall test (this echoes ADR-0002's refusal to leak
what the review is meant to test, applied to the reveal boundary rather than the scheduler).

Nothing about storage, seeding, hashing, selection, or FSRS changes. The card model already
holds arbitrary UTF-8 in `front`/`back`; the annotation lives there as ordinary text, and every
existing subsystem treats it as opaque. Furigana is a pure concern of the Renderer plus one new
pure module that parses and lays out the notation.

## User Stories

1. As a Japanese-deck author, I want to write `勉強[べんきょう]` in a card's front, so that the
   reading is attached to the kanji without a separate field or a schema change.
2. As a learner, I want the front to show only the kanji while I am recalling, so that a card
   testing a reading does not hand me the answer.
3. As a learner, I want the reading to appear directly above its kanji when I reveal, so that I
   can check my recall at a glance the way ruby text is meant to be read.
4. As a learner with a long annotated sentence, I want the reading and its kanji to wrap
   together as a unit, so that a reading never ends up on a different line from the kanji it
   belongs to.
5. As an author of an all-English (or already-plain-Japanese) card, I want it to render exactly
   as it does today, so that adding this feature costs nothing for cards that use no readings.

## Notation

We adopt Anki's furigana format verbatim, so decks are portable in both directions and the rule
is one an author may already know.

- **Form.** `base[reading]`. The square-bracketed text is the reading for the run of
  non-space characters immediately preceding the `[`.
- **Delimiter space.** A single space separates a furigana unit from the text before it, and is
  consumed (not shown) on render. Its only job is to bound the base run. Example: in
  `この 間[あいだ]` the space stops `この` from being pulled into the base, so only `間` takes the
  reading `あいだ`. A space that is not immediately before a `[…]` group is literal.
- **Okurigana and adjacent words fall out for free.** `食[た]べる` reads た over 食 with べる
  plain. `日本[にほん]語[ご]` puts にほん over 日本 and ご over 語, because each `]` bounds the
  next base run.
- **Escaping.** There is no escape character, matching Anki. An author who needs a literal
  bracket in displayed text uses the full-width forms `［ ］` (`U+FF3B`/`U+FF3D`), which are
  distinct codepoints and are never treated as markup.

**Reference parser (Anki):** the reader scans with ` ?([^ >]+?)\[(.+?)\]` and offers three
renderings: `furigana:` (ruby), `kanji:` (base only, readings and delimiter spaces stripped),
and `kana:` (readings only). Our two render modes are exactly Anki's `kanji:` (question side)
and `furigana:` (answer side). We drop the `>` from the character class because we have no HTML
to guard against, giving base = a maximal run of non-space characters ending at `[`.

Sources:
[Anki manual: field replacements](https://docs.ankiweb.net/templates/fields.html),
[obynio/anki-japanese-furigana `reading.py`](https://github.com/obynio/anki-japanese-furigana/blob/master/reading.py).

## Rendering

### Two render modes

For a given field string, the furigana module produces one of two things:

- **Base-only (question side).** Concatenate every base run and every literal (non-annotation)
  segment in order, dropping each unit's delimiter space and every `[reading]`. `勉強[べんきょう]`
  becomes `勉強`. A field with no `[…]` is returned unchanged.
- **Stacked (answer side).** Produce a **reading line** and a **base line** of equal display
  width, aligned so each reading sits centered over its base.

### Backward compatibility

If a field contains no `[…]` annotation, both modes return the original string and the Renderer
draws it exactly as today (a single `Line`, wrapped by ratatui). The stacked path is entered
only when at least one annotation is present. This keeps every existing test and every
English/plain card byte-for-byte unchanged.

### Where each mode is used

| Pane state | `front` | `back` |
|---|---|---|
| `Question` | base-only | (not shown) |
| `Answer`   | stacked   | stacked |

So the front's reading is hidden while recalling and appears above the kanji on reveal, and a
back that itself contains kanji gets the same treatment. This realizes the answer-side-only
decision.

### Stacked layout algorithm

Work in **display columns** throughout, using ratatui's `CellWidth` for `str` (already a
dependency, and the same width source the renderer and test harness use, including its
half-width-katakana dakuten adjustment). No new crate.

1. **Tokenize** the field into an ordered list of units by scanning the reference regex. Text
   between/around matches becomes plain units (`reading = None`); each match becomes a ruby unit
   `{ base, reading }`. Drop the delimiter space that precedes a match.
2. **Size each unit.** `unit_cols = max(width(base), width(reading))` (a plain unit's
   `width(reading)` is 0).
3. **Render each unit into two equal-width pieces**, centering the narrower over the wider:
   `base_piece = center(base, unit_cols)`, `read_piece = center(reading_or_blank, unit_cols)`,
   where `center` pads with spaces, left pad = `(unit_cols - width)/2`.
4. **Wrap by unit.** Greedily accumulate units into a line group until the next unit would
   exceed the pane's inner width; never split a unit across lines. Flush each group as a
   `(reading_line, base_line)` pair.
5. The Renderer emits, per group, the reading `Line` then the base `Line`, with `Wrap`
   disabled for these pre-laid-out lines (we have already wrapped). Groups are separated by a
   blank line for legibility.

Example (`front = "「 勉強[べんきょう]」は 英語[えいご]で 何[なん]と 言[い]いますか？"`,
answer side):

```
    べんきょう      えいご    なん  い
「  勉強  」は    英語  で  何  と  言いますか？
```

The reading `べんきょう` (5 kana, 10 columns) is wider than `勉強` (4 columns), so the unit is 10
columns wide and the kanji is centered beneath the reading. Plain runs (`「`, `」は`, `で`,
`と`, `いますか？`) occupy their own width with blank space above.

### Renderer integration

`PaneState` keeps carrying raw `&str` for `front`/`back` (the annotation is a rendering concern,
so the host and learning engine never see a parsed form). `renderer::draw` gains: the inner
width from `areas[0]` minus the border, a call into the furigana module per visible field, and a
branch that renders either today's single wrapped `Line` (no annotations) or the stacked,
pre-wrapped `Line` sequence (annotations present). All new logic is in `renderer.rs` and a new
`src/furigana.rs`.

## What does not change

- **Schema and storage.** `front`/`back` still hold the raw authored text, including
  annotations. No column, no migration.
- **`seed` / `parse_tsv`.** TSV parsing splits on the first tab and trims ends. Annotation text
  contains no tabs and its internal spaces are inside a line, so it survives untouched.
- **`content_hash` / dedup.** The hash is over the raw `front`+`back`. `勉強` and
  ` 勉強[べんきょう]` are therefore distinct cards, which is correct: they are different prompts.
- **Selection, FSRS, Review flow, Triggers, IPC.** Entirely unaffected.

## Admin surface

`learnwhile cards` prints `card.front` raw, so an annotated card lists as
` 勉強[べんきょう]`. This is deliberate: the admin/debug listing should show exactly what was
authored, which is what an author needs when a reading renders wrong. (If this reads as noise
later, a base-only rendering there is a one-line follow-up, not part of this spec.)

## Edge cases

- **Reading wider than base** is the normal case and is handled by `unit_cols = max(...)`.
- **A `[` with no closing `]`, or an empty `[]`,** does not match the regex and is left as
  literal text on both sides. A malformed annotation degrades to visible source, never a panic.
- **Mixed scripts in one base run** (`お茶[ちゃ]` putting ちゃ over お茶): identical to Anki. The
  author inserts a delimiter space (`お 茶[ちゃ]`) to scope the reading to 茶 only.
- **Zero-height panes / very narrow widths** already clamp via `Constraint::Min(3)`; a unit
  wider than the inner width is placed on its own group and allowed to clip, exactly as an
  over-long plain line does today.

## Testing

Following the repo's boundary-first testing rule (assert on the pane and the database, never on
internals):

- **`furigana` module unit tests** (pure, fast): base-only rendering strips readings and
  delimiter spaces; stacked rendering yields two lines of equal display width with the reading
  centered over the base; no-annotation input passes through unchanged; okurigana, adjacent
  words, and reading-wider-than-base each align correctly; a malformed `[` is left literal.
- **Host-boundary tests** (via `spawn_test_host_with_cards`, reusing the now wide-char-faithful
  `buffer_text`): a card with front ` 勉強[べんきょう]` shows `勉強` and **not** `べんきょう`
  before reveal; after reveal the pane contains `べんきょう` on the line directly above `勉強`;
  an all-English card renders identically to today (regression guard for the passthrough path).
- **Wrapping test:** a long annotated sentence in a narrow pane wraps into unit-aligned
  reading/base pairs, with no reading separated from its kanji and no line exceeding the inner
  width.

## Decisions to promote to ADRs

Two choices here are durable and belong in `docs/adr/` (drafted as ADR-0012; English first, the
zh-TW translation to follow the same pass that carried 0001-0011):

1. **Furigana is authored inline and is a pure rendering concern.** The card model and storage
   do not change; readings live in `front`/`back` as Anki-compatible notation and are parsed at
   draw time. Rejected alternative: a dedicated `reading` column, which cannot represent a field
   with several kanji runs each taking its own reading.
2. **Readings render on the answer side only.** The question side shows base kanji. This keeps a
   reading-recall card honest, consistent with ADR-0002's principle that the surface must not
   pre-empt what a review is meant to test. A config toggle was considered and deferred: it adds
   a fourth config key and a validated value for a preference no dogfooding has yet asked for.

## Out of scope

- **Automatic reading generation** (MeCab/kakasi at ingest). Lossy for names and rare readings;
  inline authored notation is the honest source of truth. Could feed the notation later.
- **Pitch-accent marks, colored readings, per-deck furigana defaults.** All layer on top of
  this notation without contradicting it.
- **Vertical text.** Out of scope for a horizontal terminal grid.
