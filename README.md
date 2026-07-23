# LearnWhile

A terminal-native spaced-repetition system that turns the time you spend waiting on an AI coding
agent into short bursts of review.

When you hand work to your agent, a card appears in a pane beside it. When the agent needs you
back, the card clears. The pane never steals focus, and nothing is ever blocked: if LearnWhile
is not running, your agent behaves exactly as it always has.

**Status: M1 — the Trigger spine.** Triggers, the pane, and fail-open all work end to end. The
card is a hardcoded placeholder; real cards, FSRS scheduling, and persistence arrive in M2 and
M3. See [`docs/milestones/`](./docs/milestones/README.md).

## Install

Requires Rust and a Unix-like OS. Windows is out of scope for v1 — the transport is a unix
domain socket ([ADR-0004](./docs/adr/0004-unix-socket-ipc-fail-open.md)).

```sh
cargo build --release
# put target/release/learnwhile somewhere on your PATH
```

## Wire up the Claude Code hook

Add this to `~/.claude/settings.json`. A Trigger opens when you hand off and closes when the
agent needs you back:

```json
{
  "hooks": {
    "UserPromptSubmit": [
      { "hooks": [{ "type": "command", "command": "learnwhile hook" }] }
    ],
    "Stop": [
      { "hooks": [{ "type": "command", "command": "learnwhile hook" }] }
    ],
    "Notification": [
      { "hooks": [{ "type": "command", "command": "learnwhile hook" }] }
    ]
  }
}
```

The same command is wired to every event; the adapter reads `hook_event_name` from the hook's
JSON on stdin and decides for itself:

| Claude Code event | Trigger |
|---|---|
| `UserPromptSubmit` | opens — you have handed control to the agent |
| `Stop` | closes — the agent finished its turn |
| `Notification` | closes — the agent wants permission or input |
| anything else | ignored — not a handoff boundary |

> **Note on the docs.** [ADR-0001](./docs/adr/0001-agent-hook-trigger-passive-surface.md) and the
> v1 spec name `PermissionRequest` and `Elicitation` as the closing events. Neither exists in
> Claude Code — the real events are `PreToolUse`, `PostToolUse`, `UserPromptSubmit`, `Stop`,
> `SubagentStop`, `SessionStart`, `SessionEnd`, `PreCompact`, and `Notification`, and the
> permission prompt surfaces as `Notification`. The code above is correct; those documents need
> amending to match.

If you would rather be explicit than let the adapter infer, `learnwhile hook --open` and
`learnwhile hook --close` force the transition regardless of the event name.

## Run

```sh
learnwhile          # or: learnwhile host
```

Put it in a pane beside your agent — a tmux or Zellij split, or a second terminal. LearnWhile
does not arrange your layout for you; that is your environment, not its business
([ADR-0003](./docs/adr/0003-long-lived-host-thin-adapters.md)).

Press `q` to quit.

## What happens if it is not running

Nothing. The hook connects to a socket that is not there, gives up instantly, and exits 0. A
crashed, wedged, or absent host cannot stall your agent — that is the property the fail-open
tests exist to defend, and it is checked against the real binary rather than a mock.

## Development

```sh
cargo test      # host-boundary tests plus the fail-open subprocess tests
cargo clippy --all-targets
cargo fmt
```

Tests boot the host in-process through one seam, then drive it by writing the same frames the
hook writes down a real unix socket, and assert on what the pane displays. No test reaches into
the open-Trigger set or the event loop directly.

## Documentation

- [`CONTEXT.md`](./CONTEXT.md) — the glossary. Terms here are load-bearing.
- [`docs/adr/`](./docs/adr/README.md) — architecture decisions, and what each one cost.
- [`docs/specs/`](./docs/specs/v1-trigger-spine-and-learning-engine.md) — the v1 spec.
- [`docs/milestones/`](./docs/milestones/README.md) — the five milestones to v1.
