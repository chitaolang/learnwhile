# LearnWhile runs as a long-lived process; adapters are thin IPC clients

**Context.** The passive side-pane surface (ADR-0001) needs a persistent viewport and holds
the rolling Session's state, but Claude Code hooks are short-lived command invocations that
can neither own a pane nor host a TUI. Something must live long enough to display cards and
hold review state.

**Decision.** LearnWhile runs as a single long-lived process hosting the Runtime Engine,
Learning Engine, Storage, and Renderer, in its own pane or terminal that the user arranges
(a tmux/zellij split or a second terminal is the user's environment, not a v1 feature). A
Trigger Adapter is a thin client — for Claude Code, a hook command — that forwards Trigger
open/close events to the running process over IPC. The long-lived process is the sole owner
of the SQLite file and the review loop.

**Consequences.** This reconciles "Terminal renderer / defer tmux+zellij": the Renderer
draws into whatever pane it is given; auto-spawning multiplexer panes is the deferred
integration. v1 requires the user to start the process and place it beside their agent; if
it is not running, adapter events must no-op (fail-open, see the IPC/fail-open decision).
Only one process may own the SQLite file at a time; concurrent agents share that one host.
The concrete IPC transport and message format are settled separately.
