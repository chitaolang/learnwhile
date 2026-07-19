# LearnWhile

A terminal-native spaced-repetition system that turns the time a developer spends
waiting on an AI coding agent into short bursts of review.

## Language

**Trigger**:
The signal that the developer has handed control to the AI agent and is now waiting. A
Trigger opens when the developer hands off and closes when the agent needs them back —
whether it finished, or needs an approval or input. While a Trigger is open, LearnWhile
may surface a card.
_Avoid_: interrupt, notification, alert

**Trigger Adapter**:
A thin, agent-specific client that translates one agent's raw events into LearnWhile
Triggers and forwards them over IPC to the long-lived LearnWhile process. Holds no learning
state itself. For Claude Code it is a hook command.
_Avoid_: plugin, listener, watcher

**Session**:
One rolling, ambient span of review activity, tied to a Trigger Adapter being connected
(roughly a work sitting) rather than to any single wait. Triggers surface cards into the
current Session; a card left unfinished when the agent returns stays in-flight in that
Session until finished or abandoned.
_Avoid_: study block, sitting, run

**Waiting**:
The aggregate state of the developer being idle on at least one agent. The Runtime holds the
set of currently-open Triggers; the developer is Waiting exactly while that set is non-empty.
A card is surfaced while Waiting and cleared when the set empties.
_Avoid_: idle, busy, blocked

**Review**:
One pass over a card. Complete once the question is shown, the answer revealed, a rating
is selected, and the result is persisted. Correctness is explicitly _not_ required for a
Review to count.
_Avoid_: attempt, quiz, test

**Learning Contract**:
An optional commitment that lets a Review outcome gate a chosen action. Always opt-in;
when absent or failed, nothing is blocked.
_Avoid_: rule, policy, guard

**Prompt Gate**:
A specific kind of Learning Contract that requires a completed Review before the agent's
next prompt is allowed to proceed.
_Avoid_: block, lock, paywall

**Fail-open**:
The default posture: a missing, unmet, or errored Learning Contract never blocks the
developer or the agent. Blocking only ever happens when a Contract is explicitly opted into.
_Avoid_: fail-safe, fail-closed
