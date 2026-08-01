# Furigana renders on the answer side only

**Context.** With readings authored inline (ADR-0012), the Renderer can show them wherever it
likes. But a large class of Japanese cards exists precisely to test *reading a kanji*: the front
is 勉強 and the thing being recalled is べんきょう. Rendering the reading above the kanji on the
question side would hand the learner the answer, defeating the card. Yet other cards use readings
as aids rather than the answer (a long sentence to translate), where hiding them helps no one.

**Decision.** The reading renders on the answer side only. On the question side, annotated text
renders base-only (plain kanji, readings and delimiter spaces stripped, Anki's `kanji:` mode);
on the answer side it renders stacked, reading over base (Anki's `furigana:` mode). This holds
for both fields, so a `back` containing kanji also gets furigana after reveal. A config toggle
(`answer-only | always | never`) was considered and deferred: it adds a fourth config key and a
validated value for a preference no dogfooding has yet asked for. Default behavior can become
configurable later without reopening this decision.

**Consequences.** A reading-recall card stays an honest test, consistent with ADR-0002's refusal
to let the surface pre-empt what a review is meant to measure, applied here to the reveal
boundary rather than to scheduling. Authors of aid-style cards who want the reading visible while
recalling cannot get that in this version; if that need shows up in dogfooding, the deferred
config toggle is the answer, and this record is where that tension was parked. The question and
answer sides now run different render modes over the same field, which the furigana module must
expose as two entry points. Spec:
[`docs/specs/furigana-ruby-display.md`](../specs/furigana-ruby-display.md).
