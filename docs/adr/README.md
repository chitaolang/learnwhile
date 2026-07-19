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

## How they relate

Several records deliberately leave a decision open for a later one to close, so reading a
record alone can overstate what was settled at the time:

- **0003** defers the IPC transport → **0004** chooses the unix socket → **0004** in turn
  defers the message format → **0007** defines the frames.
- **0005** flags that a lost Trigger close must be tolerated without saying how → **0006**
  sets the expiry policy.
- **0003** establishes thin adapters without saying how one ships → **0008** makes the
  adapter a subcommand of the host binary.

## Adding a record

Take the next number, name the file `NNNN-kebab-case-assertion.md`, and add a row above. State
the alternatives you rejected and why — that is the part a future reader needs most, because
the rejected option is usually the one they are about to propose again.
