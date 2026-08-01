# Prompt Gate: a Review can gate the next prompt

Vocabulary follows [`/CONTEXT.md`](../../CONTEXT.md). Decisions cited as ADR-NNNN from
[`docs/adr/`](../adr/). This spec is post-v1 and opt-in: with the gate off, the system behaves
exactly as v1 does, fully fail-open.

## Problem Statement

LearnWhile turns waiting time into review, but it never insists. A developer who wants the habit
can still skip every card, because the surface is passive by design (ADR-0001) and nothing is ever
blocked (ADR-0004). For some developers that passivity is the whole problem: the review they
genuinely want loses every time to the next prompt, exactly the failure the product exists to fix.

They want a commitment device: until I complete one review, hold my next prompt to the agent. The
domain model already names this. A **Learning Contract** is an opt-in commitment that lets a Review
outcome gate an action, and a **Prompt Gate** is the specific Contract that requires a completed
Review before the agent's next prompt proceeds. This spec builds that one Contract.

The tension is that this is the deliberate exception to two foundational decisions. It must not
compromise fail-open (ADR-0004) for anyone who has not opted in, it must not make the hook path
warm (ADR-0008), and it must not deadlock the developer or leave LearnWhile "in the way" of the
agent in the sense ADR-0001 forbids.

## Solution

Opt in by adding `--gate` to the `UserPromptSubmit` hook command. With the flag absent, the hook is
the same cold, fire-and-forget adapter it is today. With it present, on each prompt the hook makes
one bounded, fail-open round-trip to the host asking "do I owe a review?" If a review is owed, the
hook blocks the prompt and the Trigger does not open. If not, the prompt proceeds and the Trigger
opens as always.

A **review debt** is the state the gate reads. Handing off while a card is on screen incurs a debt.
Completing any Review clears it. If nothing was reviewable, no debt is incurred, so the gate never
traps a developer with nothing to pay. Because the block lands while the developer is not Waiting,
and a card is normally only shown while Waiting, an active gate also shows the **owed card while
idle**, so the debt is always payable on the spot. Rating it clears the debt and the next prompt
goes through.

Every uncertain path fails open: gate off, host unreachable, reply timed out, or nothing
reviewable all let the prompt proceed. The gate is a self-imposed commitment device, not a lock.
Removing the flag or quitting the host bypasses it, which is correct.

## User Stories

1. As a developer building a review habit, I want my next prompt held until I finish one card, so
   that the review I want stops losing to the next prompt.
2. As that developer, I want to clear the debt from the pane the moment I see it, even when I am
   idle between prompts, so that the block never leaves me with nothing I can do.
3. As a developer who has not opted in, I want zero change and zero added latency on the hook path,
   so that the gate costs nothing to anyone who did not ask for it.
4. As a developer whose LearnWhile is not running, I want my prompts to go through untouched, so
   that a background tool can never wedge my agent.
5. As a developer, I want turning the gate off to instantly restore the passive, never-blocking
   behavior, so that opting in is never a one-way door.

## Opt-in and the cold hook

The gate is enabled per hook registration, not in `lw config`. The `UserPromptSubmit` hook command
becomes `learnwhile hook --gate`. This is the only way to keep the gate-off path as cold as v1
(ADR-0008): the hook loads no config and cannot cheaply learn a stored setting, so the opt-in has
to travel in the invocation itself.

- **`learnwhile hook`** (no flag): unchanged. On `UserPromptSubmit` it fire-and-forgets a
  `TriggerOpen` frame and exits 0. No verdict, no round-trip.
- **`learnwhile hook --gate`**: on `UserPromptSubmit` it performs the request/response in the next
  section. On every other event it behaves exactly as the unflagged hook (the flag only changes the
  submit exchange).

The host tracks debt regardless of the flag, so enabling the gate needs no host restart. The host
learns a Session is using the gate the first time it receives a gate query, and only then does it
show the owed card while idle (see below), so a developer who never passes `--gate` never sees any
change to the idle pane.

## The gate exchange

On `UserPromptSubmit` with `--gate`, the hook and host do one request/response (ADR-0016 extends
ADR-0007's one-way frames for exactly this exchange):

1. The hook sends the open intent, marked as a gate query, and waits for a verdict within its
   existing bounded timeout.
2. The host replies **allow** or **block**:
   - **allow**: no debt is owed. The host registers the Trigger open (the handoff really happened),
     and the hook exits 0 with no blocking output. The prompt proceeds.
   - **block**: a debt is owed. The host does **not** register a Trigger open (the handoff did not
     happen), and the hook prints `{"decision":"block","reason":"Finish one review to continue."}`
     and exits 0. Claude Code blocks the prompt and shows the reason to the developer.
3. If the host does not reply in time, refuses the connection, or is not running, the hook fails
   open: it proceeds exactly as the unflagged hook would (fire-and-forget open, exit 0, no block).

Using `{"decision":"block"}` rather than exit 2 is deliberate: the `reason` is shown to the
developer, not fed to Claude as an error. The developer, not the agent, is who the gate talks to.

## Review debt

Debt is one Session-scoped boolean in the host, in memory, not persisted. It follows the lapse
queue's precedent (ADR-0010): Session-scoped in-memory review state that does not survive the host
process (ADR-0011). A restart clears it, which is acceptable and biases toward fail-open.

- **Incurred** when a card is surfaced to the pane during a wait. Selection producing the idle
  state (no due card, no new card within the cap) incurs nothing.
- **Cleared** by any completed Review, i.e. any rating. Correctness is not required and an Again
  still clears it, consistent with the **Review** and **Lapse** definitions.
- **An in-flight card** (revealed but unrated) still counts as owed. Revealing is not completing.
- **Read** by the gate exchange: block iff a debt is owed at the moment of the query.
- **Multi-agent:** one Session-wide flag. Any surfaced card arms it and one Review pays it, which
  matches "at least one review per idle stretch," not "one per agent."

## Pay the debt while idle

A hard gate blocks at `UserPromptSubmit`, which is the handoff, so the block always lands while the
developer is **not Waiting**. Today a card renders only while Waiting (`host.rs`, the
`ReviewView::Question { .. } if waiting` arms), so a blocked developer would have nothing on screen
to review and no way to summon a card, since summoning one needs a prompt, which is blocked. That
is a deadlock whose only escape is quitting LearnWhile.

So an active gate shows the owed card while idle: when the host has seen a gate query this Session
and a debt is owed and the developer is not Waiting, the pane renders the owed card (the currently
selected, in-flight card) instead of the idle state. Rating it clears the debt and the pane returns
to idle. This is the opt-in exception to passive-while-idle that the Learning Contract concept
explicitly permits (ADR-0015). It never takes foreground focus and never hides the agent's surface,
so ADR-0001's literal promise holds.

## Scope: no exemptions

The gate fires on every `UserPromptSubmit` while a debt is owed, including a prompt that answers the
agent's permission or input request. LearnWhile never hides that request and never steals focus, so
you cannot miss it (ADR-0001), but you may have to complete a review before you can answer it. This
is the accepted cost of the strict scope, and pay-while-idle keeps it from deadlocking. Exempting
replies was considered and rejected (ADR-0015): it would not remove the deadlock (a fresh prompt
after a clean `Stop` blocks while idle too, so pay-while-idle is required regardless) and it weakens
the commitment. If dogfooding shows this bites too hard, the exemption is the first thing to
reconsider.

## Fail-open matrix

| Situation | Outcome |
|---|---|
| Hook has no `--gate` | Fire-and-forget open, exit 0. No round-trip. Identical to v1. |
| `--gate`, host not running or refuses | Fail open: proceed, open, exit 0. |
| `--gate`, reply exceeds the timeout | Fail open: proceed, open, exit 0. |
| `--gate`, no debt owed | Allow: proceed, open, exit 0. |
| `--gate`, debt owed | Block: no open, print block reason, exit 0. |

Every row exits 0. The gate can only ever add a block in the last row.

## What does not change

- **Scheduling and storage.** FSRS, selection (ADR-0002), the schema, and `review_history` are
  untouched. Debt is in-memory Session state, no migration.
- **The Review flow.** Reveal on space, rate on 1 to 4, one history row per rating. The gate reads
  the outcome; it does not change how a Review works.
- **Non-submit hook events.** `Stop` and `Notification` still close a Trigger; `PreToolUse` and the
  rest are still ignored (`hook.rs`). Only the `UserPromptSubmit` exchange changes, and only under
  `--gate`.
- **The default posture.** With no `--gate` anywhere, nothing blocks, the hook stays cold, and the
  idle pane never shows a card. v1 behavior is preserved byte-for-byte.

## Edge cases

- **First prompt of a Session.** No prior handoff, no debt, so it passes.
- **Deck exhausted mid-Session.** No card is surfaced, so no new debt is incurred and the gate does
  not block.
- **Host restart with the gate on.** Debt resets to false. The first prompt after a restart passes;
  the next wait re-arms normally.
- **Reviewing while idle.** With a debt owed under an active gate, the reveal and rating keys act on
  the owed card shown in the idle pane, exactly as they do while Waiting.
- **`--gate` set but host down.** Fail open. The gate is only ever as present as the host.

## Testing

Following the repo's boundary-first rule (assert on the pane, the database, and the hook's observable
output, never on internals):

- **Gate off is invisible.** With no `--gate`, a full prompt-wait-prompt cycle never blocks and the
  idle pane never shows a card, matching v1. The hook makes no round-trip.
- **Block when owed.** Gate on, a card surfaced and left unrated, then a `UserPromptSubmit`: the hook
  emits the block verdict and the host does not open a Trigger.
- **Allow when paid.** Gate on, the surfaced card rated, then a `UserPromptSubmit`: allowed, Trigger
  opens.
- **Allow when nothing was reviewable.** Gate on, an idle wait (empty or exhausted deck), then a
  prompt: allowed.
- **Pay while idle.** Gate on, after a wait with an unpaid card the pane shows the owed card while
  not Waiting; rating it clears the debt and the next prompt is allowed.
- **Fail-open.** Gate on with the host stopped: the hook exits 0 with no block output, within its
  latency budget.
- **Latency.** The gated `UserPromptSubmit` round-trip stays within a bounded budget, asserted on the
  real binary the way ADR-0008's hook-latency test is.

## Decisions to promote to ADRs

Drafted as ADR-0014 through ADR-0016 (English first, zh-TW to follow the same pass that carried
0001 to 0013):

1. **The Prompt Gate is opt-in via a hook flag and fail-open** (ADR-0014). Enabled by `--gate`, off
   by default, cold when off, fail-open on every uncertainty. Rejected: a `lw config` key read on
   the hook path (would warm the hook, against ADR-0008).
2. **An active gate acts on the outgoing prompt and shows the owed card while idle** (ADR-0015).
   Resolves the deadlock and records the passive-while-idle exception and the no-exemption scope.
3. **The gate makes the `UserPromptSubmit` exchange request/response** (ADR-0016). Extends, does not
   replace, ADR-0007's one-way frames, for exactly this one exchange.

This spec also tightens the **Fail-open** and **Learning Contract** glossary entries so "unmet" no
longer reads as "never blocks even when opted in." An opted-in Contract blocks precisely when its
Review requirement is genuinely unmet; fail-open governs the un-evaluable and not-opted-in cases.

## Out of scope

- **Other Learning Contracts** (gating a specific tool, a per-deck daily target, a time box). The
  Prompt Gate is the first and simplest Contract; the machinery should not over-fit to it, but no
  others are built here.
- **Strictness modes** (nag-once, typed bypass). Considered during design and deferred in favor of
  the hard block. The deferred options remain the natural relief valves if the hard block bites.
- **Persisting debt across restarts.** Session-scoped and in-memory by decision, like the lapse
  queue.
- **Analytics on gate friction.** Belongs with the Analytics Engine, which is out of v1 scope
  entirely.
