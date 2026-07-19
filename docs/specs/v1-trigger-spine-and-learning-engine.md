# v1: Trigger spine + Learning Engine

Vocabulary follows [`/CONTEXT.md`](../../CONTEXT.md). Decisions cited as ADR-NNNN from
[`docs/adr/`](../adr/).

## Problem Statement

A developer working with an AI coding agent spends a large share of their day waiting —
the agent is thinking, running tools, or churning through a multi-step edit, and there is
nothing for the developer to do but watch. These waits are frequent and short: too short to
start real work, long enough to break flow. The developer fills them by drifting to a
browser tab or a chat window, and comes back worse-focused than they left.

Separately, the same developer has things they genuinely want to remember — language
details, an unfamiliar codebase's vocabulary, a framework's API. Spaced repetition is the
known answer, but it demands a deliberate daily sitting that competes with real work and
usually loses. The review habit dies not because the developer doesn't want it, but because
it never finds a slot in the day.

These two problems solve each other, and nothing currently connects them. Existing SRS tools
have no idea when the developer is waiting, and the agent has no idea the developer wants to
learn.

## Solution

LearnWhile runs as a long-lived process in a pane beside the coding agent. When the developer
hands work to the agent, a card appears in that pane; when the agent needs them back, the card
clears. Review happens in the gaps that already exist, costing no dedicated time.

The surface is deliberately passive (ADR-0001). It never steals focus, so the developer can
ignore it entirely and will still never miss a permission prompt or the agent finishing.
Nothing is ever blocked — the whole system is fail-open by construction (ADR-0004).

Scheduling is real FSRS, not a toy. Critically, LearnWhile never pulls a not-yet-due card
forward just because the developer happens to be idle (ADR-0002); it shows a due card, else
introduces a new one within a daily cap, else shows a neutral idle state. Waiting time gets
used without corrupting the spacing intervals that make spaced repetition work.

This spec covers the full v1 spine: a Claude Code Trigger Adapter, the Runtime Engine's
open-Trigger set, the Learning Engine with FSRS scheduling and Review flow, the terminal
Renderer, and SQLite storage. Manual card entry is excluded; cards are seeded from a file.

## User Stories

**Surfacing a card while Waiting**

1. As a developer, I want a card to appear in my LearnWhile pane when I submit a prompt to my
   coding agent, so that the wait I was about to waste becomes a review.
2. As a developer, I want the card to clear the moment the agent needs me back, so that my
   attention returns to the agent rather than to a half-finished review.
3. As a developer, I want the card to appear in a separate pane that never takes focus, so
   that I cannot miss a permission prompt because LearnWhile was in the way.
4. As a developer waiting on two agents at once, I want the card to stay up until *both*
   agents have come back, so that one agent finishing doesn't clear a card while I'm still
   idle on the other.
5. As a developer, I want a card that I was midway through when the agent returned to still
   be there on my next wait, so that I can finish a Review that spanned two waits.
6. As a developer, I want the pane to show a neutral idle state when I'm not Waiting, so that
   the pane's contents always tell me truthfully whether I'm expected to be reviewing.

**Doing a Review**

7. As a developer, I want to see the question side of a card first, so that I actually
   attempt recall rather than reading the answer.
8. As a developer, I want to reveal the answer with a single keypress, so that revealing
   costs me no thought during a short wait.
9. As a developer, I want to rate my recall as Again, Hard, Good, or Easy, so that the
   scheduler gets the signal it needs to space the card correctly.
10. As a developer, I want my rating persisted the instant I press it, so that a crash or a
    closed terminal never silently loses a Review.
11. As a developer, I want a Review to count as complete even when I rated myself Again, so
    that the system measures whether I showed up, not whether I was right.
12. As a developer, I want the next card to surface immediately after I rate one during the
    same wait, so that a long wait can hold several Reviews.
13. As a developer, I want to be able to simply not answer a card, so that ignoring
    LearnWhile during a wait carries no penalty and no nagging.

**Scheduling that stays honest**

14. As a developer, I want to be shown cards that are genuinely due before anything else, so
    that my waiting time goes to the reviews that matter most.
15. As a developer with nothing due, I want a new unseen card introduced instead, so that a
    wait is still useful rather than showing me an empty pane.
16. As a developer, I want a cap on how many new cards get introduced per day, so that a
    day of heavy agent use doesn't dump an entire deck on me at once.
17. As a developer, I want LearnWhile to never show me a card ahead of its due date, so that
    my FSRS intervals stay accurate and the spacing benefit isn't blunted.
18. As a developer with a small deck, I want the idle state on some waits to be normal and
    unalarming, so that I understand it as correct behavior rather than a bug.
19. As a developer, I want my scheduling data to be FSRS-shaped, so that the work I put into
    this deck isn't trapped here when import/export arrives later.

**Running the host**

20. As a developer, I want to start LearnWhile with a single command in a pane I chose, so
    that I control my own terminal layout rather than having a tool rearrange it.
21. As a developer, I want LearnWhile to keep running across many prompts and many waits, so
    that I start it once per sitting and forget about it.
22. As a developer, I want to quit LearnWhile cleanly with a keypress, so that my terminal is
    left in a sane state.
23. As a developer who restarts the host after a crash, I want it to start cleanly despite a
    stale socket file, so that a previous bad exit doesn't require manual cleanup.
24. As a developer, I want to be told clearly if a second host can't start because one is
    already running, so that I'm not confused by two panes disagreeing.

**Fail-open**

25. As a developer who hasn't started LearnWhile, I want my coding agent to behave exactly as
    it always has, so that installing the hook costs me nothing on days I don't review.
26. As a developer whose LearnWhile process has crashed, I want my agent to keep working
    normally, so that a learning tool can never take down my actual work.
27. As a developer, I want the hook to add no perceptible latency to submitting a prompt, so
    that I never feel a tax for having LearnWhile installed.
28. As a developer whose host is wedged but alive, I want the hook to give up almost
    instantly, so that a hung host can't stall my agent.
29. As a developer, I want a lost Trigger close (from an agent or host crash) to eventually
    clear, so that a phantom open doesn't pin a card up forever.

**Getting cards in**

30. As a developer trying LearnWhile, I want to seed a deck from a simple file, so that I can
    evaluate the tool without hand-entering cards through a UI that doesn't exist yet.
31. As a developer, I want re-running the seed to not duplicate cards I already have, so that
    I can iterate on my seed file safely.
32. As a developer, I want my cards and review history to survive restarts, so that the deck
    accumulates real scheduling state over days.

## Implementation Decisions

**Language and shape.** Rust. A single binary serving as both the long-lived host and the
hook client (selected by subcommand), so the hook inherits a few-millisecond startup — a
requirement, not a preference, given it fires on every prompt submission (ADR-0004). The
repo currently has no `Cargo.toml` or `src/`; this spec includes standing up the crate.

**Process topology.** One long-lived host owning the Runtime Engine, Learning Engine,
Renderer, and Storage, per ADR-0003. The user places it in a pane themselves; no multiplexer
integration. The host is sole binder of the unix socket and sole owner of the SQLite file.

**Modules.** Four internal modules behind the host boundary:

- *Runtime* — owns the open-Trigger set, accepts socket events, routes to Learning and
  Renderer, owns Session lifecycle.
- *Learning* — cards, FSRS scheduling, selection order, Review state machine. Holds no
  knowledge of sockets or terminals.
- *Renderer* — draws current card or idle state; no business logic.
- *Storage* — SQLite; the only module issuing SQL.

The Learning module must be constructible with an injected clock and an injected storage
handle. This is required for testability through the single seam (see Testing Decisions) and
is a hard constraint on its interface, not a suggestion.

**Socket protocol.** Newline-delimited JSON over a unix domain socket at
`$XDG_RUNTIME_DIR/learnwhile.sock`, falling back to a documented path when that variable is
unset. Frames are one-way from adapter to host in v1; the channel stays bidirectional-capable
so a post-v1 Prompt Gate can answer back without a second channel (ADR-0004). Frame shape:

```json
{ "v": 1, "type": "trigger_open", "adapter": "claude-code", "session": "<agent-session-id>", "at": "<rfc3339>" }
```

`type` is `trigger_open` or `trigger_close`. Trigger identity is the `(adapter, session)`
pair — ADR-0005 requires a stable identity so opens and closes pair up. The host ignores
frames it cannot parse and never lets a bad frame kill the accept loop. `v` is present so
the protocol can evolve; the host rejects unknown versions silently.

**Open-Trigger set.** A set keyed by `(adapter, session)`. `trigger_open` inserts,
`trigger_close` removes. Waiting is defined as the set being non-empty. A card surfaces on
the empty→non-empty edge and clears on the non-empty→empty edge (ADR-0005). Duplicate opens
for the same key are idempotent; a close for an unknown key is ignored.

**Lost-close recovery.** Resolves the crash-recovery gap in DESIGN_DRAFT §13; see ADR-0006.
Each open Trigger carries an expiry measured from open, defaulting to 30 minutes and held in
`config` as `trigger_expiry_seconds`. A sweep on its own periodic timer — not driven by frame
arrival — drains expired entries. Expiry is not refreshed on later frames, since no mid-turn
traffic exists to refresh from.

**Claude Code adapter.** A hook command. Opens a Trigger on `UserPromptSubmit`; closes on the
first of `Stop`, `PermissionRequest`, or `Elicitation` (ADR-0001). It reads Claude Code's hook
JSON from stdin to obtain the session id, connects with a tight timeout, writes one frame,
and exits 0 unconditionally — including on connect failure, timeout, malformed input, or
panic. It holds no learning state and never writes to storage.

**Card selection.** Strict order per ADR-0002, evaluated fresh on each surfacing: a genuinely
due card; else a new card if today's introductions are under the daily cap; else the idle
state. Never pulls a not-yet-due card forward. "Today" is resolved against the injected clock
in the user's local timezone.

**Review flow.** A state machine, not ad-hoc flags — it must survive being interrupted
mid-Review when the agent returns and resumed on a later Trigger within the same Session:

```
Idle ──surface──▶ Question ──reveal──▶ Answer ──rate(Again|Hard|Good|Easy)──▶ persist ──▶ Idle
```

A Review is complete only once persisted. Correctness is explicitly not required (CONTEXT.md).
The in-flight card is Session state: clearing the pane when the set empties must not discard
it. Ratings arrive as keypresses; reveal is a single key.

**Schema.** SQLite, tables per DESIGN_DRAFT §9. This spec resolves the card/FSRS data-model
gap from §13:

- `cards` — id, deck_id, front, back, FSRS state (`stability`, `difficulty`, `state`,
  `due`, `reps`, `lapses`, `last_reviewed_at`), `created_at`, and a content hash for
  seed idempotency.
- `review_history` — id, card_id, session_id, `reviewed_at`, `rating`, and the FSRS
  stability/difficulty before and after, plus elapsed and scheduled days. Append-only; it is
  the audit trail the deferred Analytics Engine will read, so it records enough to
  reconstruct scheduler state.
- `decks` — id, name. v1 creates and uses a single default deck; decks exist in the schema so
  post-v1 doesn't need a migration.
- `config` — key/value. Holds the daily new-card cap and the Trigger expiry.

Migrations run on host startup.

**FSRS.** Use an existing FSRS implementation rather than hand-rolling one; the `fsrs-rs`
crate is the obvious candidate but its API surface should be confirmed before committing.
Default parameters are fine for v1 — no optimization pass.

**Renderer.** `ratatui`. Draws the current card or the idle state. This spec resolves the
idle-pane-content gap from §13: the idle state shows due-today and new-remaining counts plus
the next due time — enough to tell "nothing due" apart from "not waiting," and enough that
the pane is never blank.

**Card seeding.** Since manual entry is out of scope, a `seed` subcommand ingests a
tab-separated front/back file into the default deck, skipping rows whose content hash already
exists. This is a developer affordance to make v1 usable, explicitly **not** the deferred
Anki-compatible Import/Export feature, and should not accrete format support.

## Testing Decisions

**What makes a good test here.** Tests assert only on what a developer could observe: what
the pane displays, and what ended up in the database. No test reaches into the open-Trigger
set, the Review state machine, or the selection function directly. A test that breaks when
those internals are restructured — but where the developer's experience is unchanged — is a
bad test and should be rewritten.

**One seam: the host boundary.** Confirmed with the developer. Tests boot the host in-process
with three injected dependencies — a temp SQLite path, a temp socket path, and a controllable
clock — then drive it by dialing the **real unix socket** and writing the **same frames the
hook writes**. This exercises the actual protocol and the real open-Trigger set rather than a
mock of either. The Learning Engine is deliberately *not* given its own seam, so no test can
pass while the wired-up path is broken.

The injected clock is not optional: FSRS is time-dependent, and due-date behavior tested
against a real clock is non-deterministic. Time is advanced explicitly to test due dates and
the daily cap's rollover.

**Two input channels at that seam.** The socket carries Triggers; keypresses carry reveal and
rating. Both are inputs to the same host boundary, so the event loop needs a test-reachable
way to inject key events — this is a design constraint on the loop, and should be settled
early rather than retrofitted.

**Two observation surfaces.** `ratatui`'s `TestBackend` renders to an in-memory cell buffer,
which tests assert against for what the developer sees. Direct SQLite reads confirm what
persisted. Both are external to the modules under test.

**What is tested.** Trigger open surfaces a card and close clears it; two overlapping Triggers
keep the card up until both close; a lost close expires and drains; an in-flight card survives
the agent returning and resumes on the next Trigger; the full reveal-and-rate flow writes a
`review_history` row and advances the card's due date; selection follows due → new → idle
strictly; a not-yet-due card is never surfaced; the daily cap holds and rolls over; the idle
state shows correct counts; seeding is idempotent.

**Fail-open is tested at the adapter separately.** The hook is invoked as a subprocess with no
host running, a refused socket, and a deliberately wedged socket; in every case it must exit 0
within its timeout. This is the one place a subprocess test is warranted, because "the real
binary exits 0" is precisely the claim, and it cannot be made about an in-process harness.

**Prior art.** None — this is the repo's first code, so these tests set the pattern. That
argues for getting the harness right early: a single well-built `spawn_test_host` helper is
the highest-leverage thing in this spec.

**Not tested.** FSRS interval math itself — that belongs to the upstream crate's test suite,
and re-testing it here would couple us to its internals. We test that we call it and persist
what it returns.

## Out of Scope

- **Manual card entry** (CLI or in-TUI). Cards come from `seed`. The card-add UX gap in
  DESIGN_DRAFT §13 stays open.
- **Contract Engine** — Learning Contracts and Prompt Gates. v1 never blocks anything. The
  socket stays bidirectional-capable so this doesn't require rework.
- **Analytics Engine.** `review_history` is recorded richly enough to feed it later, but
  nothing reads it.
- **Import / Export** (Anki TSV, CSV, JSON). The `seed` subcommand is not this.
- **Additional Trigger Adapters** (Codex, OpenCode). The protocol is adapter-keyed so they
  drop in without touching the Runtime.
- **Auto-spawning tmux/Zellij panes and a Desktop Renderer.** The user arranges their own
  layout (ADR-0003).
- **Multiple decks in the UI.** Schema supports them; v1 uses one default deck.
- **FSRS parameter optimization.** Defaults only.
- **Windows.** Unix sockets aren't native on older Windows (ADR-0004); v1 is Unix-only.
- **Multi-host / multi-user.** One host owns the socket and the database.

## Further Notes

**Three decisions were promoted to ADRs.** The Trigger expiry policy (ADR-0006), the
newline-delimited JSON frame format (ADR-0007), and the single-binary-with-subcommands topology
(ADR-0008) began life in this spec and now live in `docs/adr/`, which is the durable record.
Where this spec and those ADRs disagree, the ADRs win.

**Nothing here contradicts an existing ADR.** The spec commits ADR-0004's deferred "concrete
IPC transport and message format," and resolves ADR-0005's explicitly-flagged lost-close
tolerance — both are gaps those ADRs left open by design, not reversals.

**Sequencing.** The spine should land before the Learning Engine: Trigger → socket → set →
Renderer with a hardcoded card proves the riskiest, least-reversible part (does the hook fire
when we think, does fail-open hold) while it's still cheap to change. FSRS and storage are
conventional work that can follow with confidence.

**The hypothesis under test.** DESIGN_DRAFT §3 frames v1 as testing *will developers review
during AI waits?* Worth noting this build can't fully answer that — `review_history` holds
the data, but nothing surfaces it until the Analytics Engine exists. Reading it by hand is
fine for a first dogfood; just don't mistake shipping this for having the answer.
