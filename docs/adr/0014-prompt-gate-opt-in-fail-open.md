# The Prompt Gate is opt-in via a hook flag and fail-open

**Context.** LearnWhile is passive (ADR-0001) and fail-open (ADR-0004): nothing ever blocks the
developer or the agent. Some developers want the opposite for themselves, a commitment device that
holds the next prompt until they complete one Review. The domain model already names this as the
**Prompt Gate**, a kind of **Learning Contract**. Building it must not compromise fail-open for
anyone who has not opted in, and must not warm the hook path, which loads no config and runs on
every prompt (ADR-0008). The hook cannot cheaply read a stored setting, so it cannot learn from
`lw config` whether the gate is on without paying a cost on every submit for every user.

**Decision.** The gate is opted into per hook registration, by changing the `UserPromptSubmit` hook
command to `learnwhile hook --gate`. Without the flag the hook is byte-for-byte the v1 cold path: a
fire-and-forget `TriggerOpen`, no round-trip, no verdict. With the flag, and only on
`UserPromptSubmit`, the hook makes one bounded, fail-open round-trip to the host for an allow/block
verdict. The gate never blocks when the host is unreachable, the reply times out, or nothing was
reviewable. Rejected: an `lw config` key read on the hook path, which would either warm the hook
(against ADR-0008) or force an always-on round-trip that taxes developers who never opted in.

**Consequences.** Opt-in lives in `settings.json`, not `lw config`, so it is a little less
discoverable and toggling it means editing the hook command. The host tracks review debt regardless
of the flag, so enabling the gate needs no host restart. The gate is trivially bypassable by
removing the flag or quitting the host, which is correct: it is a self-imposed commitment device,
not a lock, and fail-open (ADR-0004) is never sacrificed to it. Spec:
[`docs/specs/prompt-gate.md`](../specs/prompt-gate.md).
