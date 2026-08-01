# An active Prompt Gate acts on the outgoing prompt and shows the owed card while idle

**Context.** A hard Prompt Gate (ADR-0014) blocks at `UserPromptSubmit`, which is the handoff, so
the block always lands while the developer is not Waiting. But a card renders only while Waiting
(`host.rs`, the `ReviewView::Question { .. } if waiting` arms). A blocked developer would therefore
have nothing on screen to review and no way to summon a card, because summoning one needs a prompt,
which is blocked. That is a deadlock whose only escape is quitting LearnWhile. Separately, the gate
acts on the developer's outgoing prompt, which can be a reply to the agent's permission or input
request, brushing against ADR-0001's promise that LearnWhile is never in the way.

**Decision.** While a gate is active and a Review is owed, the pane shows the owed card even when the
developer is not Waiting, so the debt is always payable on the spot; any rating clears it and the
pane returns to idle. The gate applies to every outgoing prompt with no exemption, including replies
to the agent. ADR-0001's literal guarantee is kept: the agent's request is never hidden and the pane
never takes foreground focus. The developer may, however, have to complete a Review before
answering. Rejected: exempting replies to the agent, which would not remove the deadlock (a fresh
prompt after a clean `Stop` blocks while idle too, so pay-while-idle is required either way) and
would weaken the commitment.

**Consequences.** The pane is no longer strictly passive-while-idle when a gate is active: it
surfaces the owed card outside Waiting. This is confined to the opted-in gate case, which the
Learning Contract concept explicitly permits, and the host enters it only after it has seen a gate
query this Session, so a developer who never passes `--gate` sees no change to the idle pane.
Blocking a reply to the agent is possible and accepted; pay-while-idle keeps it from deadlocking. If
dogfooding shows the no-exemption scope bites too hard, the exemption is the first relief valve to
reconsider. Spec: [`docs/specs/prompt-gate.md`](../specs/prompt-gate.md).
