# M7 — Prompt Gate

**Goal.** A developer can opt into a Prompt Gate so that, until they complete one Review, their next
prompt to the agent is held, with the owed card shown in the pane so the debt is always payable, and
with the gate off nothing changes and nothing ever blocks.

The second post-v1 slice. Scope is [`docs/specs/prompt-gate.md`](../specs/prompt-gate.md); it adds
one opt-in Learning Contract and touches the hook, the host state, and the socket reply path, but
not scheduling or storage.

## Demo

1. Enable the gate: set the `UserPromptSubmit` hook to `learnwhile hook --gate`.
2. Submit a prompt. A card appears. Do not review it. Let the agent finish.
3. Try to submit the next prompt. It is blocked with `Finish one review to continue.`, and the pane
   shows the owed card even though you are idle.
4. Rate the card. Submit again. It goes through, and a Trigger opens as normal.
5. Turn the gate off (drop `--gate`). Repeat the cycle: nothing ever blocks, and the idle pane never
   shows a card. Behavior is identical to v1.
6. With the gate on, quit the host and submit a prompt. It goes straight through. The gate is only
   ever as present as the host.

## What ships

**UI.** The block reason shown to the developer, and the owed-card-while-idle pane state so a blocked
developer always has something to rate.

**Backend.** The `--gate` hook variant and its bounded, fail-open round-trip; Session-scoped review
debt in the host; the socket reply path that answers the gate query; and fail-open on every
uncertain path.

## Sub-tasks

1. **Review debt state.** A Session-scoped, in-memory boolean in the host, set when a card is
   surfaced during a wait and cleared on any rating. Follows the lapse queue's precedent (ADR-0010):
   Session-scoped review state that dies with the host process (ADR-0011). Host-boundary testable on
   its own.
2. **Owed-card-while-idle pane.** When the host has seen a gate query this Session and a debt is
   owed and the developer is not Waiting, render the owed card instead of the idle state. Gate this
   strictly on "a gate query was seen," so a non-gate developer's idle pane never changes (ADR-0015).
3. **Socket reply path.** The listener answers a gate query with allow or block on the single event
   loop (ADR-0009), extending ADR-0007's one-way frames for this one exchange only (ADR-0016). On
   block, the host must not register the Trigger open.
4. **The `--gate` hook.** A new match arm (no `clap`, ADR-0008). On `UserPromptSubmit` with the flag,
   do the bounded round-trip: on allow, proceed and let the open register; on block, print
   `{"decision":"block","reason":"..."}` and exit 0; on any timeout, refusal, or host-down, fail
   open exactly as the unflagged hook. Every other event behaves as the unflagged hook.
5. **Wire the surface.** The block reason wording, and the idle-pane hint that a review unblocks the
   next prompt.

## Tests

- With no `--gate`, a full prompt-wait-prompt cycle never blocks, the idle pane never shows a card,
  and the hook makes no round-trip. v1 is preserved.
- Gate on, a card surfaced and left unrated, then a submit: the hook emits the block verdict and the
  host does not open a Trigger.
- Gate on, the card rated, then a submit: allowed, Trigger opens.
- Gate on, an idle wait (empty or exhausted deck), then a submit: allowed.
- Gate on, after a wait with an unpaid card: the pane shows the owed card while not Waiting, rating
  it clears the debt, and the next submit is allowed.
- Gate on with the host stopped: the hook exits 0 with no block output, within its latency budget.
- The gated `UserPromptSubmit` round-trip stays within a bounded budget, measured on the real binary
  the way ADR-0008's hook-latency test is.

## Exit criteria

- The M7 demo can be performed by someone who has not read this repo.
- With the gate off, behavior and the hook path are byte-for-byte v1: no block, no round-trip, no
  idle-pane change.
- The gate never blocks when the host is unreachable, the reply times out, or nothing was
  reviewable.
- A blocked developer can always clear the debt from the pane. There is no state in which the gate
  demands a review that cannot be performed.

## Not in this milestone

- **Other Learning Contracts** (gating a specific tool, a per-deck daily target, a time box). The
  Prompt Gate is the first Contract; no others are built here.
- **Strictness modes** (nag-once, typed bypass). Deferred in favor of the hard block; they are the
  natural relief valves if the hard block bites (spec §Scope).
- **A `lw config` opt-in.** Opt-in stays the `--gate` flag to keep the gate-off hook cold (ADR-0014).
- **Persisting debt across restarts.** Session-scoped and in-memory by decision.
- **zh-TW translations** of the spec and ADR-0014 to 0016. They follow the same pass that carried
  0001 to 0013.

## Decisions this relies on

ADR-0014 (opt-in via `--gate`, fail-open), ADR-0015 (the gate acts on the outgoing prompt and shows
the owed card while idle), ADR-0016 (the `UserPromptSubmit` exchange becomes request/response), which
extends ADR-0007. It is constrained by ADR-0001 (the pane never takes focus or hides the agent, even
under a gate), ADR-0004 (fail-open is never sacrificed), ADR-0008 (the gate-off hook stays cold), and
ADR-0009 (the reply is answered on the one event loop).

## Risks

**Fail-open must be airtight.** A bug that blocks when it should not, for example when the host is
merely slow, is worse than the gate not firing. Bias every uncertain path to allow, and test the
host-down and timeout paths explicitly, not just the happy path.

**The gated round-trip is on every prompt.** A local unix socket exchange is sub-millisecond, but it
is now on the developer's critical path when the gate is on. Keep the timeout tight, fail open on
exceed, and assert the budget on the real binary (ADR-0008).

**Blocking a reply to the agent may frustrate.** The no-exemption scope (ADR-0015) can hold up a
developer answering a permission prompt. Pay-while-idle prevents a deadlock, but if dogfooding shows
it bites, reconsider the reply exemption before adding complexity elsewhere.

**Pay-while-idle must rate the same card the Review flow owns.** The owed card shown in the idle pane
has to be the in-flight card the rating keys act on, or a developer could rate one card while looking
at another. Drive it through the existing Review state, not a parallel copy.
