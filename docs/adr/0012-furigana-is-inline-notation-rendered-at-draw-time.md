# Furigana is inline notation, parsed and rendered at draw time

**Context.** A Japanese card's front is often kanji whose reading the learner is acquiring, and
that reading wants to sit above the kanji as ruby text (furigana). LearnWhile stores a card as a
`front`/`back` pair of opaque UTF-8 columns, and every subsystem (seed, hashing, selection,
FSRS, Review) treats that text as data it never interprets. A reading has to attach to specific
kanji *within* a field, and a single field can hold several kanji runs each taking its own
reading, so "one reading per card" cannot represent it.

**Decision.** Readings are authored inline in the existing `front`/`back` text using
Anki-compatible furigana notation (`base[reading]`, space-delimited), and are parsed only at
render time by the Renderer and one pure furigana module. The stored text is the raw authored
string, annotations included. Rejected: a dedicated `reading` column, which cannot express
multiple independently-read kanji runs in one field and would force a schema migration for a
purely visual concern. Rejected: automatic reading generation (MeCab/kakasi) at ingest, which is
lossy for names and rare readings.

**Consequences.** No schema change, no migration, and the card model, seeding, `content_hash`,
selection, and FSRS are all untouched. The annotation rides along as ordinary text, so
`content_hash` treats `勉強` and ` 勉強[べんきょう]` as distinct cards, which is correct: they are
different prompts. `learnwhile cards` lists the raw annotated front, which is what an author
debugging a reading wants to see. Decks stay portable to and from Anki. All new logic lives in
`renderer.rs` plus `src/furigana.rs`; nothing else in the system learns the notation exists.
Spec: [`docs/specs/furigana-ruby-display.md`](../specs/furigana-ruby-display.md).
