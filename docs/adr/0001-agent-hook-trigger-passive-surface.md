# Triggering is driven by AI-agent lifecycle hooks, surfaced passively

**Context.** LearnWhile's premise is converting AI-waiting time into review. We considered
terminal idle-detection and manual invocation, but chose to drive Triggers from the coding
agent's own lifecycle events (via a per-agent Trigger Adapter, e.g. Claude Code hooks),
because that is the only source that actually distinguishes "waiting on the AI" from other
idle time.

**Decision.** A card surfaces on an agent Trigger and clears when the agent returns. The
default surface is a **non-blocking side pane** (tmux/zellij) that never steals foreground
focus — so the developer cannot miss a permission prompt or the agent completing. Blocking
is available only when a **Learning Contract** (e.g. a Prompt Gate) is explicitly opted into;
absent one, the system is fail-open and nothing is ever blocked.

**Consequences.** Integration is agent-specific: each supported agent needs its own Trigger
Adapter. The passive default means the habit is unenforced unless the user opts into a
Contract. Per-adapter hook details (which events map to a Trigger, how "the agent returned"
is detected) are deferred to each adapter's design.
