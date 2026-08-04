# Milestones

The record of how LearnWhile was built: five vertical slices from an empty repo to an installable
v1, plus two post-v1 slices. Vocabulary follows the glossary in [`/CONTEXT.md`](../../CONTEXT.md);
decisions are cited as ADR-NNNN from [`docs/adr/`](../adr/); scope is
[`docs/specs/v1-trigger-spine-and-learning-engine.md`](../specs/v1-trigger-spine-and-learning-engine.md).

Every milestone is a vertical slice: it ships UI *and* backend and ends with something a developer
can run and judge. The order follows the spec's sequencing argument, spine first, because the spine
carries the least reversible risk (does the hook fire where we think, does fail-open hold) while it
is still cheap to change; FSRS and storage follow as conventional work.

| # | Milestone | What a developer can do at the end |
|---|---|---|
| [M1](#m1-trigger-spine) | Trigger spine | See a card appear in a pane while the agent works, and clear when it returns |
| [M2](#m2-cards-and-reviews) | Cards and Reviews | Seed a real deck and complete Reviews that persist and reschedule |
| [M3](#m3-honest-scheduling) | Honest scheduling | Trust that what they're shown is genuinely due, capped, and never pulled forward |
| [M4](#m4-session-continuity) | Session continuity | Resume a Review interrupted by the agent, and re-attempt a card they failed |
| [M5](#m5-hardening-and-install) | Hardening and install | Install it, run it every day, and diagnose it when it misbehaves |
| [M6](#m6-furigana-display) | Furigana display | Author Japanese cards with readings and see furigana over the kanji on reveal |
| [M7](#m7-prompt-gate) | Prompt Gate | Opt in to hold the next prompt until one Review is done, with the owed card always payable |

M1 through M5 are v1; M6 and M7 are post-v1. Each section below records what the milestone
delivered and the decisions that constrain it. The step-by-step acceptance walkthroughs live in the
by-hand checklists under [`docs/test-plan/`](../test-plan/).

## M1 Trigger spine

**Goal.** A card appears in the pane the moment a prompt is submitted and clears the moment the
agent needs the developer back, and killing the host leaves Claude Code entirely unaffected.

**Delivers.** The `hook` and host subcommands, dispatched by matching `std::env::args()` rather than
`clap` (ADR-0008). A unix socket carrying NDJSON `TriggerFrame { v, type, adapter, session, at }`
(ADR-0007), the socket path resolved from `$XDG_RUNTIME_DIR/learnwhile.sock` with a documented
fallback. An event loop with three producer threads (socket accept, terminal input, sweep tick)
feeding one `mpsc` channel, with the main thread owning all state (ADR-0009). The open-Trigger set
keyed by `(adapter, session)`, where Waiting is the set being non-empty; the pane surfaces on the
empty-to-non-empty edge and clears on the reverse (ADR-0005). A per-entry expiry sweep (ADR-0006). A
passive `ratatui` pane that never takes focus (ADR-0001).

**Guarantees.** Every hook failure path exits 0: no socket, refused, timeout, malformed stdin, or
panic. The hook does no SQLite, TUI, or logging work. A line that does not parse, is oversized, or
carries an unknown `v` is dropped without killing the accept loop. A duplicate open is idempotent
and a close for an unknown key is ignored. Overlapping Triggers hold the card until both close. A
Trigger whose close never arrives expires on the sweep. The quit key restores the terminal.

**Relies on.** ADR-0001, 0003, 0004, 0005, 0006, 0007, 0008, 0009. (The close event in practice is
`Notification`, not the `PermissionRequest`/`Elicitation` the early spec named; see the root README.)

## M2 Cards and Reviews

**Goal.** A developer seeds a deck from a file and completes real Reviews during their waits,
question then reveal then rate, with every rating persisted and the card rescheduled by FSRS.

**Delivers.** A `rusqlite` (`bundled`) storage module, the only module issuing SQL, with the
database under the XDG data directory. One migration against `PRAGMA user_version`, run on host
startup, creating `cards`, `decks`, `review_history`, and `config` and seeding the default deck and
config defaults. Trigger expiry moves out of M1's constant into `trigger_expiry_seconds` in config
(ADR-0006). The `seed` subcommand parses TSV front/back pairs, dedupes on a content hash, and stays
TSV-only. A `Clock` trait injected into the Learning engine so due dates are testable. FSRS
constructed with `DEFAULT_PARAMETERS` and `desired_retention` from config, mapping the four ratings
onto `next_states`. A `Question -> Answer -> persist -> Idle` Review state machine that persists on
the rating keypress. An append-only `review_history` row per rating (rating, stability and
difficulty before and after, elapsed days, scheduled days).

**Guarantees.** Reseeding the same file inserts nothing. A reveal-and-rate flow writes exactly one
`review_history` row and advances the due date. All four ratings persist, including Again. A rating
survives a host restart. Migration is idempotent. The question side never shows the answer until the
reveal key arrives.

**Relies on.** ADR-0003, 0006, 0009.

## M3 Honest scheduling

**Goal.** The pane shows a genuinely due card if one exists, otherwise a new card within the day's
cap, otherwise an informative idle state, and never a card pulled forward ahead of its due date.

**Delivers.** The real selection policy, replacing M2's placeholder, evaluating ADR-0002's order
(due, then new if under the cap, then idle) fresh on each surfacing. "Due" is measured against the
injected clock, never SQLite's own time functions. The daily cap reads `new_cards_per_day`, resolves
"today" in the user's local timezone via `chrono`'s `Local`, and derives the count from
`review_history` so a restart cannot lose or double it. The idle state shows due-today count,
new-remaining count, and the next due time. After a rating persists, selection re-runs immediately
so one long wait can hold several Reviews. The empty deck renders sensibly rather than panicking.

**Guarantees.** Selection follows due then new then idle strictly. A not-yet-due card is never
surfaced, even with nothing else in the deck and the developer Waiting. The cap holds within a day,
rolls over at local midnight, and survives a restart. The idle counts are correct, including with an
empty deck. Nothing in the codebase can surface a card ahead of its due date (M4 adds the one
bounded exception).

**Relies on.** ADR-0002, and the config defaults in the v1 spec.

## M4 Session continuity

**Goal.** A card the developer was halfway through when the agent returned is still there on the next
wait, and a card they failed comes back for a second attempt before the sitting ends.

**Delivers.** A rolling Session tied to a Trigger Adapter being connected, spanning many
Waiting/idle cycles. In-flight Review state that outlives a pane clear: when the open-Trigger set
empties, the Renderer stops drawing but Learning keeps its position, and the next Trigger re-surfaces
the card in the state it was left (a revealed answer stays revealed). An in-memory, Session-scoped
lapse queue: a card rated Again is appended, a card rated anything else is removed (ADR-0010).
Selection order becomes lapse, then due, then new, then idle. Re-attempts are ordinary Reviews
(`next_states` with `days_elapsed` of zero), identifiable in `review_history` by the zero elapsed
days with no extra column. Abandoning a card carries no timeout and no nagging. The queue and
in-flight card die with the Session and the process, written nowhere.

**Guarantees.** An interrupted revealed card resurfaces revealed. A card rated Again returns later in
the same Session ahead of due and new, and stops once rated Good. The lapse queue never survives a
restart and never reaches SQLite, so `cards.state` keeps its `{new, review}` domain. Rating Again
still writes a `review_history` row and reschedules. An unfinished card is neither discarded nor
escalated across many cycles.

**Relies on.** ADR-0010, ADR-0002 as narrowed by it, ADR-0005.

## M5 Hardening and install

**Goal.** A developer can install LearnWhile, leave it running every day, and work out what happened
when something misbehaves, without reading the source.

**Delivers.** A `tracing` file log under the XDG state directory with bounded (daily-rotated) growth,
recording every discarded frame with its reason (unparseable, unknown `v`, oversized) and any
producer-thread death (ADR-0007, ADR-0009). Single-instance detection: one host owns the socket and
database (ADR-0003), and a second refuses with a message naming the running one rather than a bind
error. Stale-socket recovery that connects before unlinking so a live socket is never removed (the
same code path as single-instance detection, from the opposite direction). A hook-latency assertion
measured on the real binary (ADR-0008). Terminal restore on `SIGINT` and `SIGTERM`, not just the quit
key. A README sufficient for the whole install-and-run path. A release profile tuned for startup
time, not binary size.

**Guarantees.** A second host refuses without disturbing the first's socket or database. A stale
socket does not prevent startup, and a socket with a live listener is never unlinked. Each
discarded-frame reason produces a log line and the host keeps accepting valid frames. The hook stays
within its latency budget. The terminal is restored after `SIGINT` and `SIGTERM`.

**Relies on.** ADR-0001, 0003, 0004, 0007, 0008, 0009.

## M6 Furigana display

**Goal.** A developer authors a Japanese card with inline readings and sees furigana (the reading
stacked over its kanji) on reveal, while the question side shows only the kanji and no non-Japanese
card changes at all. Scope is
[`docs/specs/furigana-ruby-display.md`](../specs/furigana-ruby-display.md); it changes only how a
card is drawn.

**Delivers.** One pure module, `src/furigana.rs`, with no schema, storage, seeding, hashing,
selection, or FSRS change. A tokenizer over the Anki reader rule (` ?([^ >]+?)\[(.+?)\]`) that drops
the single delimiter space before a match and leaves a malformed or unclosed `[` literal. A base-only
mode (`kanji:`) for the question side that concatenates bases and drops readings, returning an
un-annotated field unchanged. A stacked layout (`furigana:`) for the answer side where each unit is
`max(width(base), width(reading))` display columns via ratatui's `CellWidth`, the narrower centered
over the wider, wrapped so a unit never splits. Renderer integration: `Question` draws `front`
base-only, `Answer` draws `front` and `back` stacked with `Wrap` disabled, and `PaneState` keeps
carrying raw `&str`.

**Guarantees.** A field with no `[…]` renders identically to pre-M6 on both sides. Base-only strips
the reading and the delimiter space. Stacked rendering yields two lines of equal display width with
the reading centered, including okurigana, adjacent words, and reading-wider-than-base. A malformed
annotation stays literal and never panics. The reading is never visible on the question side. A long
sentence in a narrow pane wraps into unit-aligned reading/base pairs.

**Relies on.** ADR-0012 (inline notation parsed at draw time), ADR-0013 (answer side only), ADR-0001.

## M7 Prompt Gate

**Goal.** A developer opts into a Prompt Gate so that, until they complete one Review, their next
prompt is held, with the owed card shown so the debt is always payable, and with the gate off nothing
changes and nothing ever blocks. Scope is [`docs/specs/prompt-gate.md`](../specs/prompt-gate.md); it
adds one opt-in Learning Contract and touches the hook, host state, and socket reply path, not
scheduling or storage.

**Delivers.** Session-scoped, in-memory review debt in the host, set when a card is surfaced during a
wait and cleared on any rating, dying with the process (ADR-0011). An owed-card-while-idle pane that
renders the owed card instead of the idle state, but only once a gate query has been seen this
Session, so a non-gate developer's idle pane never changes (ADR-0015). A socket reply path answering
the gate query on the single event loop, extending ADR-0007's one-way frames for this one exchange
(ADR-0016); on a block the host does not register the Trigger open. The `--gate` hook match arm (no
`clap`, ADR-0008) doing a bounded round-trip on `UserPromptSubmit`: allow proceeds, block prints
`{"decision":"block","reason":"..."}` and exits 0, and any timeout, refusal, or host-down fails open
exactly as the unflagged hook.

**Guarantees.** With the gate off, behavior and the hook path are byte-for-byte v1: no block, no
round-trip, no idle-pane change. An unrated card blocks the next submit and no Trigger opens; a rated
card allows it. An idle or exhausted wait allows the submit. The owed card shows while idle and rating
it clears the debt. With the host stopped the hook exits 0 within budget. The round-trip stays within
a budget asserted on the real binary. The gate never blocks when the host is unreachable, the reply
times out, or nothing was reviewable, and the debt is always clearable from the pane.

**Relies on.** ADR-0014 (opt-in via `--gate`, fail-open), ADR-0015 (acts on the outgoing prompt,
shows the owed card while idle), ADR-0016 (the `UserPromptSubmit` exchange becomes request/response),
constrained by ADR-0001, 0004, 0008, 0009.

## Deferred beyond these milestones

Named here so the boundary stays visible rather than being rediscovered:

- **The Analytics Engine.** `review_history` is recorded richly enough to feed it, but nothing reads
  it yet. Shipping v1 is not the same as having answered the hypothesis.
- **Import/Export beyond TSV `seed`.** No CSV, JSON, or Anki formats; `seed` is a convenience, not an
  importer.
- **Furigana toggles and generation.** A visibility config (`answer-only | always | never`) is
  deferred by ADR-0013, and automatic reading generation is deferred in favor of authored notation
  as the source of truth (ADR-0012).
- **Other Learning Contracts and gate strictness modes.** The Prompt Gate is the first Contract;
  per-tool gates, per-deck targets, time boxes, nag-once, typed bypass, and a `config` opt-in are all
  deferred (ADR-0014).
- **Windows, multi-host/multi-user, and FSRS parameter optimization.** All out of v1: unix sockets
  (ADR-0004), single ownership (ADR-0003), and defaults-only FSRS respectively.
