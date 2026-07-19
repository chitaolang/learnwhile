# LearnWhile Specification (Draft v1.1)

> A terminal-native spaced repetition learning system optimized for
> developer idle time.

## 1. Vision

LearnWhile helps developers turn AI waiting time into focused learning.

The project is **Anki-compatible, not Anki-dependent**.

------------------------------------------------------------------------

## 2. Design Philosophy

-   Learning Engine is the core.
-   Runtime is independent from learning logic.
-   Event-driven architecture.
-   Fail-open by default.
-   Learning Contracts are optional.
-   Analytics measures outcomes, not behavior.

------------------------------------------------------------------------

## 3. Core Concepts

-   Card
-   Deck
-   Review
-   Review Event
-   Learning Contract
-   Session
-   Trigger
-   Renderer

------------------------------------------------------------------------

## 4. High-level Architecture

``` text
LearnWhile
├── Runtime Engine
├── Learning Engine
├── Contract Engine
├── Analytics Engine
├── Trigger Adapter
├── Renderer
├── Storage
└── Import / Export
```

------------------------------------------------------------------------

## 5. Runtime Engine

**Purpose**

Coordinates the entire application lifecycle. It connects triggers,
learning, rendering, storage, and contracts without containing learning
logic.

**Responsibilities**

-   Session lifecycle
-   Event routing
-   Trigger coordination
-   Renderer coordination

------------------------------------------------------------------------

## 6. Learning Engine

**Purpose**

Owns the learning domain, including spaced repetition, card management,
and review scheduling.

**Responsibilities**

-   Cards
-   Decks
-   FSRS
-   Review flow
-   Due card selection

------------------------------------------------------------------------

## 7. Contract Engine

**Purpose**

Evaluates optional learning commitments and determines whether learning
goals have been satisfied before allowing specific actions.

**Responsibilities**

-   Prompt Contract
-   Time Contract
-   Session Contract
-   Daily Contract

Prompt Gate is a Prompt Contract.

A review is complete when:

1.  Question shown
2.  Answer revealed
3.  Rating selected (Again / Hard / Good / Easy)
4.  Review persisted

Correctness is not required.

------------------------------------------------------------------------

## 8. Analytics Engine

**Purpose**

Transforms application events into meaningful metrics that help users
understand their learning effectiveness and AI usage.

**Responsibilities**

-   Learning metrics
-   AI waiting utilization
-   Contract completion
-   Dashboard metrics

Read-only.

------------------------------------------------------------------------

## 9. Storage

**Purpose**

Persists all application data and provides a single source of truth.

SQLite tables:

-   cards
-   review_history
-   decks
-   config

------------------------------------------------------------------------

## 10. Trigger Adapter

**Purpose**

Converts external tools and AI agents into standardized runtime events.

Examples:

-   Claude Code
-   Codex
-   OpenCode

------------------------------------------------------------------------

## 11. Renderer

**Purpose**

Presents the user interface without containing business logic.

Supported implementations:

-   Terminal
-   tmux
-   Zellij
-   Desktop (future)

------------------------------------------------------------------------

## 12. Import / Export

**Purpose**

Converts LearnWhile data to and from external formats while keeping the
Learning Engine independent.

Supported formats:

-   Anki TSV
-   CSV
-   JSON

------------------------------------------------------------------------

## 13. Future Roadmap

-   Insights Engine
-   AI-generated cards
-   Obsidian
-   Notion
-   Cloud Sync
-   Team Learning
