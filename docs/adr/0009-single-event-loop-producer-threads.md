# The host is one event loop fed by producer threads; no async runtime

**Context.** The host has three independent input sources that all mutate the same state.
Trigger frames arrive on the unix socket (ADR-0004, ADR-0007) and change the open-Trigger set.
Keypresses drive the Review state machine — reveal, then a rating. And the expiry sweep must
run on its own periodic timer, which ADR-0006 makes a hard requirement rather than a detail:
hanging the sweep off frame arrival would mean a phantom open with no subsequent traffic never
drains, the exact case that ADR exists to address. All three touch the open-Trigger set, the
in-flight card, or the pane, so something has to serialise them. Two further constraints
narrow the choice: the v1 spec's testing decision requires both the socket and key input to be
reachable at the host boundary, and ADR-0008 requires the `hook` path to stay cold.

**Decision.** One producer thread per source — socket accept, terminal input, sweep tick —
each translating its source into a single `Event` type and sending it on one
`std::sync::mpsc` channel. The main thread owns every piece of host state and consumes events
serially; no host state is shared between threads and nothing is behind a lock. Producer
threads hold no state and make no decisions; they translate and send. Tests hold a clone of
the `Sender`, which is how key events get injected without a terminal attached. Rejected:
**tokio with `select!`**, which is the idiomatic answer for socket servers but brings a
runtime for a workload of a handful of frames per agent turn, and — more to the point —
puts an async runtime inside the binary whose hook path ADR-0008 exists to keep cold, making
that discipline a thing to remember rather than a thing that is structurally true. Also
rejected: **a single thread polling** `crossterm::event::poll` with a timeout alongside a
non-blocking accept, which needs no threads but collapses sweep timing, input latency, and
idle CPU into one timeout constant that cannot be tuned for all three; and **shared state
behind a `Mutex`** touched from each thread, which reintroduces exactly the interleavings the
channel removes.

**Consequences.** State ownership is single-threaded, so there is no lock discipline to get
wrong and no deadlock to reason about. Event ordering is total, which tests may rely on: a
frame written before a keypress is handled before it. The cost is three threads for a
near-idle process, which does not matter for a local developer tool. Two obligations follow.
First, a producer thread must not be able to take the host down: per ADR-0007 a malformed
frame may not kill the accept loop, so the failure boundary is the individual connection, not
the thread — and a producer thread that does die must be visible in the log file rather than
silently starving its input. Second, the channel is unbounded, so back-pressure is invisible;
this is acceptable only because the frame volume is negligible by ADR-0007's own reasoning,
and it is the assumption to revisit before any adapter emits intermediate progress frames.
When a post-v1 Prompt Gate needs the host to answer back on the same connection (ADR-0004),
the `Event` carries the accepted stream so the main loop can reply — the shape accommodates
this without a second channel, but v1 does not implement it.
