# One binary is both the long-lived host and the hook client

**Context.** ADR-0003 splits the system into a long-lived host and thin, agent-specific
adapters, but leaves open how an adapter is actually shipped and invoked. The constraint that
decides it is latency: the Claude Code adapter runs on every `UserPromptSubmit`, directly in
the path of the developer submitting a prompt, and ADR-0004 requires it to return near-instantly
so that LearnWhile can never be felt as a tax on the agent. Whatever the adapter is, its
cold-start cost is paid by the developer many times an hour.

**Decision.** Ship one binary that selects its role by subcommand: a default invocation runs
the host (socket, Runtime, Learning, Renderer, Storage); `hook` acts as the Trigger Adapter,
reading Claude Code's hook JSON on stdin and writing a single frame; `seed` ingests cards. A
Rust binary starts in single-digit milliseconds, comfortably inside the budget. Rejected: a
separate adapter binary, which is cleaner in principle but produces two artifacts that can
drift out of version with each other and doubles what a user must install; and a shell script
driving `nc` or `socat`, which needs no build but depends on tools that are not reliably
present, vary in whether they speak unix sockets at all, and offer poor control over send
timeouts and exit codes — the two things the adapter absolutely must get right.

**Consequences.** The `hook` path must stay cold: no SQLite open, no TUI initialisation, no
config load beyond resolving the socket path. This is a standing discipline rather than a
one-time change, since shared startup code is the natural place for initialisation to
accumulate and would silently reintroduce the latency this ADR exists to avoid. Keeping the
hook path's work minimal is worth asserting in a test that measures it, not just intending.
Binary size grows because the hook carries the host's code, which is irrelevant for a locally
installed developer tool. A single artifact makes host/adapter version skew impossible on one
machine, so the `v` field in ADR-0007 earns its place only for future third-party or non-Rust
adapters — which is still reason enough to keep it.
