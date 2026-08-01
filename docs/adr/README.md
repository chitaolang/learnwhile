# Architecture Decision Records

One file per decision, numbered in the order taken. Titles are written as assertions, so the
list below can be read as a summary of what LearnWhile has committed to. Each record follows
the same three-part shape: **Context** (the tension that forced a choice), **Decision** (what
was chosen, and what was rejected), **Consequences** (what this costs).

Vocabulary follows the glossary in [`/CONTEXT.md`](../../CONTEXT.md).

## Records

| # | Decision |
|---|---|
| [0001](./0001-agent-hook-trigger-passive-surface.md) | Triggering is driven by AI-agent lifecycle hooks, surfaced passively |
| [0002](./0002-card-selection-protects-scheduler.md) | Card selection never early-reviews; it protects the scheduler |
| [0003](./0003-long-lived-host-thin-adapters.md) | LearnWhile runs as a long-lived process; adapters are thin IPC clients |
| [0004](./0004-unix-socket-ipc-fail-open.md) | Adapters reach the host over a unix socket; fail-open by construction |
| [0005](./0005-runtime-open-trigger-set.md) | The Runtime tracks an open-Trigger set; "waiting" means it is non-empty |
| [0006](./0006-trigger-expiry-drains-phantom-opens.md) | Open Triggers expire, so a lost close cannot pin the card up forever |
| [0007](./0007-ndjson-trigger-frames.md) | Adapters send newline-delimited JSON frames identified by adapter and session |
| [0008](./0008-single-binary-subcommands.md) | One binary is both the long-lived host and the hook client |
| [0009](./0009-single-event-loop-producer-threads.md) | The host is one event loop fed by producer threads; no async runtime |
| [0010](./0010-lapsed-cards-requeue-within-session.md) | A lapsed card returns within the same Session; the ban on pulling forward holds across them |
| [0011](./0011-session-is-host-process-lifetime.md) | A Session is the lifetime of the host process |
| [0012](./0012-furigana-is-inline-notation-rendered-at-draw-time.md) | Furigana is inline notation, parsed and rendered at draw time |
| [0013](./0013-furigana-renders-on-the-answer-side-only.md) | Furigana renders on the answer side only |

## How they relate

Several records deliberately leave a decision open for a later one to close, so reading a
record alone can overstate what was settled at the time:

- **0003** defers the IPC transport → **0004** chooses the unix socket → **0004** in turn
  defers the message format → **0007** defines the frames.
- **0005** flags that a lost Trigger close must be tolerated without saying how → **0006**
  sets the expiry policy.
- **0010** makes the lapse queue die "with the Session" without saying what bounds a Session →
  **0011** defines a Session as the host process lifetime.
- **0003** establishes thin adapters without saying how one ships → **0008** makes the
  adapter a subcommand of the host binary.
- **0012** makes furigana an inline-notation rendering concern without saying *where* the
  reading shows → **0013** confines it to the answer side, extending ADR-0002's
  "don't pre-empt the review" from the scheduler to the reveal boundary.
- **0004**, **0006** and the Review flow each introduce an input the host must handle, without
  saying how they coexist → **0009** serialises all three onto one event loop.

One record narrows an earlier one:

- **0010** carves a Session-bounded exception into **0002**'s ban on surfacing a not-yet-due
  card. Read 0002 alone and the ban looks absolute; it is absolute across Sessions and days,
  which is the part that protects the scheduler.

## Adding a record

Take the next number, name the file `NNNN-kebab-case-assertion.md`, and add a row above. State
the alternatives you rejected and why — that is the part a future reader needs most, because
the rejected option is usually the one they are about to propose again.
