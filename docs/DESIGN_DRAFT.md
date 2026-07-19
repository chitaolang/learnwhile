# LearnWhile Specification (Draft v2)

> A terminal-native spaced repetition learning system that turns the time a
> developer spends waiting on an AI coding agent into short bursts of review.

Vocabulary in this document follows the glossary in [`/CONTEXT.md`](../CONTEXT.md).
Load-bearing decisions are recorded as ADRs under [`docs/adr/`](./adr/) and are
cited inline as ADR-NNNN.

---

## 1. Vision

LearnWhile helps developers turn AI waiting time into focused learning.

The project is **Anki-compatible, not Anki-dependent**.

---

## 2. Design Philosophy

- The Learning Engine is the core; the runtime is independent of learning logic.
- Event-driven: the Runtime routes Triggers and review events between components.
- **Fail-open by default** — a missing, unmet, or errored Learning Contract, or an
  absent host, never blocks the developer or the agent (ADR-0004).
- Learning Contracts are optional and opt-in.
- Analytics measures outcomes, not behavior. Read-only.

---

## 3. v1 Scope

v1 is the smallest slice that tests the core hypothesis — *will developers review
during AI waits?* — with cards entered by hand.

**In v1:**

- Runtime Engine
- Learning Engine (FSRS scheduling, card selection, Review flow)
- One Trigger Adapter: **Claude Code** (a hook client)
- Terminal Renderer (draws into whatever pane the user provides)
- Storage (SQLite)
- Manual card entry

**Deferred (post-v1):**

- Contract Engine (Learning Contracts, Prompt Gate)
- Analytics Engine
- Import / Export (Anki TSV, CSV, JSON)
- Additional Trigger Adapters (Codex, OpenCode)
- Auto-spawning tmux / Zellij pane integration and a Desktop Renderer

---

## 4. High-level Architecture

LearnWhile runs as a **single long-lived process** (the host) that the user places
in its own pane or second terminal. Trigger Adapters are thin, agent-specific
clients that forward Triggers to it over a unix socket (ADR-0003, ADR-0004).

``` text
pane A: Claude Code ──hook (Trigger Adapter)──▶ unix socket
                                                   │
pane B: LearnWhile host (long-lived) ◀────────────┘
        ├── Runtime Engine        (open-Trigger set, event routing)
        ├── Learning Engine        (FSRS, card selection, Review flow)
        ├── Renderer               (passive card pane)
        └── Storage                (SQLite — sole owner)

Deferred: Contract Engine · Analytics Engine · Import/Export
```

The user arranges the split (tmux / Zellij / a second terminal); the multiplexer is
the user's environment, not a v1 component (ADR-0003).

---

## 5. Runtime Engine

**Purpose**

Coordinates the application lifecycle and routes events between adapters, learning,
rendering, and storage, without containing learning logic.

**Responsibilities**

- Own the **open-Trigger set**: the developer is **Waiting** exactly while the set is
  non-empty (ADR-0005). A card is surfaced while Waiting and cleared when the set
  empties — even across multiple concurrent agents.
- Accept Trigger open/close events from adapters over the unix socket.
- Session lifecycle (see §11).
- Renderer coordination.
- Tolerate a lost close (agent or host crash) so the set does not leak a phantom
  open — via a per-Trigger timeout and/or a session-end sweep (open gap, §13).

---

## 6. Learning Engine

**Purpose**

Owns the learning domain: spaced repetition, card management, and Review scheduling.

**Responsibilities**

- Cards and (post-v1) Decks.
- FSRS scheduling.
- Card selection on each Trigger, in strict order (ADR-0002):
  1. a genuinely **due** card; else
  2. introduce a **new** card, up to a daily new-card cap; else
  3. signal the idle/stats state.
  It never pulls a not-yet-due card forward, keeping FSRS intervals honest.
- Review flow. A **Review** is complete once the question is shown, the answer
  revealed, a rating is selected (Again / Hard / Good / Easy), and the result is
  persisted. Correctness is **not** required.

---

## 7. Trigger Adapter — Claude Code (v1)

**Purpose**

Translate one agent's raw events into Triggers and forward them to the host.

**Behavior**

- A Trigger **opens** on `UserPromptSubmit` (covering the agent's thinking and tool
  time) and **closes** on the first of `Stop`, `PermissionRequest`, or `Elicitation`
  — i.e. the moment the agent needs the developer back (ADR-0001).
- The adapter is a hook command: it connects to the host's unix socket, sends the
  event fire-and-forget with a tight timeout, and swallows all errors (always exits
  0). A down or absent host is a silent no-op — **fail-open** (ADR-0004).
- The adapter holds no learning state.

---

## 8. Renderer (v1: Terminal)

**Purpose**

Present the card without containing business logic.

**Behavior**

- Draws a **passive** card into its own pane; it never steals foreground focus, so the
  developer cannot miss a permission prompt or the agent completing (ADR-0001).
- Shows the current card while Waiting; shows a neutral idle/stats state otherwise.

Deeper integrations (auto-spawning tmux / Zellij panes, a Desktop Renderer) are
deferred.

---

## 9. Storage

**Purpose**

Persist all data and provide a single source of truth. The host is the **sole owner**
of the SQLite file (ADR-0003); only one host may run at a time.

SQLite tables:

- cards
- review_history
- decks
- config

---

## 10. Session

One rolling, ambient **Session** tied to a Trigger Adapter being connected (roughly a
work sitting), not to any single wait. Triggers surface cards into the current
Session; a card left unfinished when the agent returns stays in-flight until finished
or abandoned. A Session spans many Waiting/idle cycles of the open-Trigger set.

---

## 11. Deferred Engines (post-v1)

**Contract Engine.** Evaluates optional **Learning Contracts** (Prompt / Time /
Session / Daily). A **Prompt Gate** is a Learning Contract that requires a completed
Review before the agent's next prompt proceeds — the one case where LearnWhile blocks,
and only when opted in. The bidirectional unix socket already anticipates carrying a
Review result back to the adapter for this (ADR-0004).

**Analytics Engine.** Transforms events into learning metrics, AI-waiting
utilization, and Contract completion. Read-only.

**Import / Export.** Converts LearnWhile data to and from Anki TSV, CSV, and JSON,
keeping the Learning Engine independent of external formats.

---

## 12. Future Roadmap

- Insights Engine
- AI-generated cards
- Obsidian / Notion integration
- Cloud Sync
- Team Learning

---

## 13. Known Gaps

Resolved architecture aside, these v1 details are still open (do not block the spine):

- **Crash recovery** — concrete lost-close policy: per-Trigger timeout value and/or a
  session-end sweep to drain the open-Trigger set (ADR-0005).
- **Manual card-add UX** — CLI command vs in-TUI form; and whether v1 exposes Decks or
  a single default deck.
- **Idle-pane content** — what the Renderer shows when nothing is due.
- **Card / FSRS data model** — per-card FSRS state fields and the `review_history`
  shape.
