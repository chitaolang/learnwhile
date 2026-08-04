# LearnWhile

*English | [繁體中文](./README.zh-TW.md)*

A terminal-native spaced-repetition system that turns the time you spend waiting on an AI coding
agent into short bursts of review.

When you hand work to your agent, a card appears in a pane beside it. When the agent needs you
back, the card clears. The pane never steals focus, and nothing is ever blocked: if LearnWhile
is not running, your agent behaves exactly as it always has.

**Status: v1 feature-complete (M1–M5), plus two post-v1 additions (M6–M7).** Seed a deck from a
file and do real Reviews during your waits: question, reveal, rate. Ratings persist and cards are
rescheduled by FSRS. Beyond v1, Japanese cards can carry furigana readings shown over the kanji on
reveal (M6), and an opt-in Prompt Gate can hold your next prompt until you finish one Review (M7).
The sections below cover each feature; [`docs/milestones/`](./docs/milestones/README.md) has the
full history.

## Install

Requires Rust and a Unix-like OS. Windows is out of scope for v1 — the transport is a unix
domain socket ([ADR-0004](./docs/adr/0004-unix-socket-ipc-fail-open.md)).

```sh
cargo build --release
# put target/release/learnwhile somewhere on your PATH
```

Or let the install script build it and copy the binary onto your PATH (`~/.local/bin` by default,
or set `PREFIX`):

```sh
./scripts/install.sh
# PREFIX=/usr/local/bin ./scripts/install.sh   # a system-wide location, may need sudo
```

To remove it later, run `./scripts/uninstall.sh`. Add `--purge` to also delete your cards, review
history, logs, and socket.

### Or let your AI agent install it

LearnWhile is for people who already work with an AI coding agent, so the quickest setup is to hand
the whole thing to that agent. From a clone of this repo, paste this prompt into Claude Code (or your
agent of choice):

> Install LearnWhile from this repository for me:
> 1. Run `./scripts/install.sh` to build the release binary and put it on my PATH.
> 2. Add the LearnWhile hook to my `~/.claude/settings.json` for the `UserPromptSubmit`, `Stop`, and
>    `Notification` events, each running the command `learnwhile hook`. Merge into any hooks I
>    already have instead of overwriting them, and show me the diff before saving.
> 3. Seed the N5 deck: `learnwhile seed data/anki-jlpt/n5.tsv`.
>
> Do not start the host yourself, it is a full-screen TUI I will run in my own pane. When you are
> done, tell me to restart this session and run `learnwhile` beside my agent.

A few things to expect:

- **Review the `settings.json` change.** The agent is editing your global agent config; skim the diff
  before accepting, and confirm it merged into your existing hooks rather than replacing them.
- **Restart the agent session** once the hooks are added, so Claude Code loads them.
- **You start the host, not the agent.** `learnwhile` is an interactive full-screen pane; run it
  yourself in a split beside your agent (see [Run](#run)). The agent cannot drive it for you.
- Want the Prompt Gate? Tell the agent to use `learnwhile hook --gate` for `UserPromptSubmit`
  instead (see [Prompt Gate](#prompt-gate-optional)).

## Command reference

`learnwhile` is one binary with several subcommands. Each has a fuller walkthrough in the sections
below.

| Command | What it does |
|---|---|
| `learnwhile`<br>`learnwhile host` | Start the review pane (the long-lived host). |
| `learnwhile hook` | The Claude Code hook adapter: read the event from stdin, then open or close a Trigger. Always exits 0. |
| `learnwhile hook --open` | Force a Trigger open, ignoring the event name. |
| `learnwhile hook --close` | Force a Trigger close, ignoring the event name. |
| `learnwhile hook --gate` | Hook adapter with the Prompt Gate on: hold your next prompt until one Review is done. |
| `learnwhile seed <file.tsv>` | Import cards from a tab-separated file. Re-running skips cards already present. |
| `learnwhile extract <notes.csv> [out-dir]` | Build JLPT seed decks from an anki-jlpt-decks source export. `out-dir` defaults to the current directory. |
| `learnwhile config` | List every setting and its value. |
| `learnwhile config set <key> <value>` | Change one setting. Rejects an unknown key or an unusable value. |
| `learnwhile cards` | List every card and how it is scheduled. |

With no subcommand, `learnwhile` starts the host. An unknown one prints the usage line and exits
non-zero:

```sh
$ learnwhile wat
learnwhile: unknown subcommand "wat"
usage: learnwhile [host|hook|seed|config|cards|extract]
```

## Wire up the Claude Code hook

Add this to `~/.claude/settings.json`. A Trigger opens when you hand off and closes when the
agent finishes its turn:

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
| `Stop` | closes — the agent finished its whole turn (the only close) |
| `Notification` | ignored — fires mid-turn on permission prompts and idle waits, so it is not the end of your wait |
| anything else | ignored — not a handoff boundary |

## Seed a deck

Cards come from a tab-separated file, one card per line: the front, a tab, then the back.

```
What does FSRS stand for?	Free Spaced Repetition Scheduler
Capital of France	Paris
```

Load it into your deck:

```sh
learnwhile seed cards.tsv
```

Re-running is safe. A card already in your deck is skipped, so you can edit the file and seed
again without duplicating anything. This is a convenience for trying LearnWhile, not an
Anki-style importer, so it takes TSV and nothing else. The database lives under your XDG data
directory (`$XDG_DATA_HOME/learnwhile/`, or `~/.local/share/learnwhile/`).

### Furigana for Japanese cards

A card's text may carry Anki-style furigana: `勉強[べんきょう]`, where the bracketed kana is the
reading for the kanji immediately before it, with a space to bound a run where needed
(`この 間[あいだ]`). The reading is hidden on the question side and appears stacked over its kanji
when you reveal, so a "read this kanji" card stays an honest test. Cards with no brackets render
exactly as before. See
[`docs/specs/furigana-ruby-display.md`](./docs/specs/furigana-ruby-display.md).

### JLPT decks

Ready-made Japanese decks live in [`data/anki-jlpt/`](./data/anki-jlpt/): one seed file per JLPT
level (`n5.tsv` through `n1.tsv`, about 10,600 cards total), each already in the furigana notation
above. Seed a level the same way as any other TSV:

```sh
# seed the N5 deck (807 cards); start here for the gentlest set
learnwhile seed data/anki-jlpt/n5.tsv
# → 807 added, 0 skipped (already present)

# stack more levels whenever you like; re-running is safe, duplicates are skipped
learnwhile seed data/anki-jlpt/n4.tsv
learnwhile seed data/anki-jlpt/n1.tsv

# confirm they landed
learnwhile cards
```

Seeding a whole level does not flood you: the `new_cards_per_day` cap (20 by default) still meters
how many new cards the pane introduces each day, so even the 4,044-card N1 deck feeds in gradually.

Those files are generated from the [`5mdld/anki-jlpt-decks`](https://github.com/5mdld/anki-jlpt-decks)
source export by the `extract` subcommand:

```sh
learnwhile extract notes.csv [out-dir]   # writes n1.tsv .. n5.tsv; out-dir defaults to .
```

It reads the deck's tab-separated source and writes one `front<TAB>back` file per level, ready for
`seed`. The source deck is by **egg rolls**, licensed
[CC BY-NC 4.0](https://creativecommons.org/licenses/by-nc/4.0/), and the derived decks carry the
same license (credit egg rolls, non-commercial use only). Full attribution is in
[`data/anki-jlpt/README.md`](./data/anki-jlpt/README.md).

## Inspecting and tuning

`config` and `cards` read the same database, whether or not the host is running. The settings
`config` lists and `config set` changes are:

| Key | Default | What it does |
|---|---|---|
| `trigger_expiry_seconds` | `1800` | how long a Trigger stays open before a lost close expires it |
| `desired_retention` | `0.9` | the FSRS target recall probability |
| `new_cards_per_day` | `20` | the daily cap on new-card introductions |

The host reads config at startup, so restart it after a change. `config set` refuses an unknown
key or a value the host could not use, so a typo fails here rather than at the next launch.

## Run

```sh
learnwhile          # or: learnwhile host
```

Put it in a pane beside your agent — a tmux or Zellij split, or a second terminal. LearnWhile
does not arrange your layout for you; that is your environment, not its business
([ADR-0003](./docs/adr/0003-long-lived-host-thin-adapters.md)).

When a card appears, press space to reveal the answer, then rate your recall: `1` Again, `2`
Hard, `3` Good, `4` Easy. The available keys are always shown along the bottom of the pane. Your
rating is saved the instant you press it and the card is rescheduled by FSRS. Press `q` to quit.

## What the pane shows

On each wait, LearnWhile picks what to show in a fixed order:

1. A card you failed earlier in this sitting (rated Again), offered again for a second try.
2. Otherwise, a card that is genuinely due.
3. Otherwise, a new card — up to a daily limit, so a busy day never dumps the whole deck on you.
4. Otherwise, an idle pane showing how many cards are due, how many new ones remain today, and
   when the next card comes due.

A card is never shown before its due date, so your FSRS intervals stay honest. The one exception
is a card you just failed this sitting: it comes back the same day for a re-attempt and stops once
you rate it anything other than Again.

If the agent comes back while you are mid-review, the card is not lost. It is waiting in the same
state — a revealed answer stays revealed — on your next wait. Ignoring a card costs nothing: there
is no timer and no nagging. A sitting lasts as long as the host runs; restarting it starts fresh.

## Prompt Gate (optional)

By default nothing is ever blocked. If you want a commitment device, point your `UserPromptSubmit`
hook at `learnwhile hook --gate` instead:

```json
"UserPromptSubmit": [
  { "hooks": [{ "type": "command", "command": "learnwhile hook --gate" }] }
]
```

With the gate on, your next prompt is held until you complete one Review. The owed card stays in the
pane even while idle, so you can always clear it: rate it, and your prompt goes through. Without the
flag the hook is unchanged, and the gate never blocks when LearnWhile is not running, is slow to
answer, or has nothing to review. It is a self-imposed nudge, not a lock. See
[`docs/specs/prompt-gate.md`](./docs/specs/prompt-gate.md).

## What happens if it is not running

Nothing. The hook connects to a socket that is not there, gives up instantly, and exits 0. A
crashed, wedged, or absent host cannot stall your agent — that is the property the fail-open
tests exist to defend, and it is checked against the real binary rather than a mock.

## Where things live, and resetting

LearnWhile keeps three things under your XDG directories:

| What | Where |
|---|---|
| Database (cards, review history) | `$XDG_DATA_HOME/learnwhile/learnwhile.db`, else `~/.local/share/learnwhile/` |
| Log file (rotated daily) | `$XDG_STATE_HOME/learnwhile/host.log.<date>`, else `~/.local/state/learnwhile/` |
| Socket | `$XDG_RUNTIME_DIR/learnwhile.sock`, else `/tmp/learnwhile.sock` |

The host is the sole owner of the database and the socket
([ADR-0003](./docs/adr/0003-long-lived-host-thin-adapters.md)). Starting a second host refuses
with a message naming the running one, rather than two panes disagreeing. If a host is killed and
leaves a stale socket behind, the next start recovers automatically, with no manual cleanup.

When something misbehaves, the log is where to look: every discarded frame (with the reason) and
any producer-thread failure is recorded there. The pane stays passive and silent by design
([ADR-0001](./docs/adr/0001-agent-hook-trigger-passive-surface.md)), so the log carries the
diagnostics rather than the screen.

To reset, stop the host and delete the database. This clears your cards, review history, and any
config changes. The next `learnwhile seed` or host start recreates an empty database with the
default settings. The command below works whether or not `$XDG_DATA_HOME` is set:

```sh
rm -f "${XDG_DATA_HOME:-$HOME/.local/share}/learnwhile/learnwhile.db"
```

## Development

```sh
cargo test      # host-boundary tests plus the fail-open subprocess tests
cargo clippy --all-targets
cargo fmt
```

## Documentation

- [`CONTEXT.md`](./CONTEXT.md) — the glossary. Terms here are load-bearing.
- [`docs/adr/`](./docs/adr/README.md) — architecture decisions, and what each one cost.
- [`docs/specs/`](./docs/specs/v1-trigger-spine-and-learning-engine.md) — the v1 spec.
- [`docs/milestones/`](./docs/milestones/README.md) — the five milestones to v1.
