# M6 — Furigana display

**Goal.** A developer can author a Japanese card with inline readings and see furigana (the
reading stacked over its kanji) when they reveal the answer, while the question side still shows
only the kanji, and no non-Japanese card changes at all.

The first post-v1 slice. Scope is
[`docs/specs/furigana-ruby-display.md`](../specs/furigana-ruby-display.md); it changes only how a
card is drawn, never how one is stored, selected, or scheduled.

## Demo

1. Seed a card whose front is ` 勉強[べんきょう]` and whose back is `study`, alongside an
   ordinary English card, into a real deck.
2. Run the host and trigger a wait so the Japanese card comes up. The question side shows
   `勉強`. The reading `べんきょう` is nowhere on screen.
3. Reveal. The reading `べんきょう` now sits on the line directly above `勉強`, centered over it,
   and `study` shows below.
4. Bring up the English card the same way. It renders exactly as it did before this milestone,
   one line, no reading line, no change.
5. Seed and reveal a long annotated sentence in a narrow pane. It wraps into reading-over-base
   line pairs, and no reading is ever separated from the kanji it belongs to.

## What ships

**UI.** Two render modes in the Renderer: base-only for the question side (kanji shown, readings
and their delimiter spaces stripped), and a stacked reading-over-base layout for the answer side,
column-aligned and wrapped so a unit never splits. Cards with no annotation render exactly as
today.

**Backend.** One new pure module, `src/furigana.rs`, that parses the Anki-compatible notation and
lays it out in display columns. No schema, no migration, no change to storage, seeding, hashing,
selection, or FSRS. The card model already carries the annotation as opaque text.

## Sub-tasks

1. **Tokenizer.** Scan a field with the Anki reader rule (` ?([^ >]+?)\[(.+?)\]`, minus the HTML
   guard we do not need) into an ordered list of units: plain runs and ruby units carrying a base
   and a reading. Drop the single delimiter space that precedes a match. A malformed or unclosed
   `[` matches nothing and stays literal. Pure, unit-tested, leaves the build green.
2. **Base-only rendering (`kanji:` mode).** Concatenate base runs and literal segments in order,
   dropping every reading. A field with no annotation returns unchanged. This is the question
   side and the backward-compatibility guarantee in one function.
3. **Stacked layout (`furigana:` mode).** For each unit compute `unit_cols = max(width(base),
   width(reading))` in display columns via ratatui's `CellWidth` for `str` (already a dependency,
   the same width source the renderer and the M6-era test harness use). Center the narrower over
   the wider. Emit a reading line and a base line of equal width. Unit-tested against explicit
   expected strings.
4. **Unit-aware wrapping.** Given the pane's inner width, greedily group units into
   `(reading_line, base_line)` pairs without splitting a unit, and separate groups with a blank
   line. Unit-tested at narrow widths.
5. **Renderer integration.** `renderer::draw` derives the inner width from `areas[0]` minus the
   border and, per visible field, either draws today's single wrapped `Line` (no annotation
   present) or emits the pre-wrapped stacked `Line` sequence with `Wrap` disabled. `Question`
   renders `front` base-only; `Answer` renders `front` and `back` stacked. `PaneState` keeps
   carrying raw `&str`, so the host and Learning Engine never see a parsed form.

## Tests

- A field with no `[…]` renders identically to the pre-M6 output, on both sides. (Regression
  guard for every existing English card and every existing test.)
- Base-only rendering strips readings and the delimiter space: ` 勉強[べんきょう]` yields `勉強`.
- Stacked rendering yields two lines of equal display width with the reading centered over its
  base, including okurigana (`食[た]べる`), adjacent words (`日本[にほん]語[ご]`), and the common
  reading-wider-than-base case.
- A malformed annotation (`[` with no `]`, empty `[]`) is left literal and never panics.
- Through the host boundary: a card with front ` 勉強[べんきょう]` shows `勉強` and not
  `べんきょう` before reveal, and after reveal the pane contains `べんきょう` on the line directly
  above `勉強`.
- A long annotated sentence in a narrow pane wraps into unit-aligned reading/base pairs, no
  reading separated from its kanji, no line exceeding the inner width.

## Exit criteria

- The M6 demo can be performed by someone who has not read this repo.
- Every English or already-plain card renders byte-for-byte as it did before M6.
- No reading is visible on the question side of any card.
- The card model, schema, seeding, and scheduler are untouched by this milestone.

## Not in this milestone

- **A visibility config toggle** (`furigana = answer-only | always | never`). Deferred by
  ADR-0013 until dogfooding asks for it; answer-only is the fixed behavior here.
- **Automatic reading generation** (MeCab/kakasi at ingest). Inline authored notation is the
  source of truth (ADR-0012); generation could feed that notation in a later slice.
- **Pitch-accent marks, colored readings, per-deck furigana defaults.** All layer on this
  notation without contradicting it.
- **Base-only rendering in `learnwhile cards`.** The admin listing keeps showing the raw
  annotated front, which is what an author debugging a reading wants (spec §Admin surface).
- **zh-TW translations of ADR-0012 and ADR-0013.** They follow the same pass that carried
  0001-0011, not this milestone.

## Decisions this relies on

ADR-0012 (furigana is inline notation parsed at draw time, so nothing but the Renderer changes),
ADR-0013 (readings render on the answer side only), which extends ADR-0002's refusal to let the
surface pre-empt what a review tests, and ADR-0001 (the pane stays passive, so this is a
rendering change with no new input or focus behavior).

## Risks

**Terminal and font disagreement on CJK width.** The layout is computed in East Asian Width
columns via `CellWidth`. A terminal that renders ambiguous-width characters or emoji at a
different width will drift the alignment. Mitigation: use the one width source the whole renderer
already uses so the buffer and the layout agree, and accept that a mis-configured terminal is
outside what this milestone can fix.

**Base scoping for mixed-script runs.** Without a delimiter space, `お茶[ちゃ]` puts ちゃ over the
whole `お茶`. This is identical to Anki and is the author's call to make with a space
(`お 茶[ちゃ]`). Mitigation: follow Anki's rule exactly and document it in the spec rather than
inventing a smarter scoper that would diverge from Anki.

**A reading far wider than its base inflates spacing.** `unit_cols = max(...)` widens the unit to
the reading, spreading the surrounding text. This is inherent to ruby and is accepted, not
worked around.
