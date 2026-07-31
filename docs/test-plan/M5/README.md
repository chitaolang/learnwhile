# M5 manual test plan — Hardening and install

A by-hand checklist for the [M5 milestone](../../milestones/M5-hardening-and-install.md): install
LearnWhile, leave it running, and work out what happened when something misbehaves — without
reading the source.

The automated suite (`cargo test`) covers the logic. This plan is the demo from the milestone,
performed against the real binary. It assumes you have done the [M2](../M2/README.md) plan once so
seeding and running are familiar.

## Setup

Do this once, in **every terminal** you use. Isolated XDG homes so nothing touches your real data,
logs, or socket.

```sh
cd /Users/timlin/learnwhile
cargo build --release
alias lw="$PWD/target/release/learnwhile"
export XDG_DATA_HOME=/tmp/lw-m5/data
export XDG_STATE_HOME=/tmp/lw-m5/state
export XDG_RUNTIME_DIR=/tmp/lw-m5/run
mkdir -p "$XDG_RUNTIME_DIR"
alias showlog='cat "$XDG_STATE_HOME"/learnwhile/host.log.* 2>/dev/null'
SOCK="$XDG_RUNTIME_DIR/learnwhile.sock"
```

Reset everything with `rm -rf /tmp/lw-m5` (recreate `$XDG_RUNTIME_DIR` afterward).

Seed one card so the host has content:

```sh
printf 'front1\tback1\n' > /tmp/deck.tsv
lw seed /tmp/deck.tsv
```

You will use two terminals: **A** runs the host, **B** runs the probes. Both need the Setup block.

## 1. Install and run from the README

- [ ] `lw host` in Terminal A starts and shows the idle pane (`Not waiting`, `Due now: 0 ...`). No error, no stale-state prompt.
- [ ] The log was created: in Terminal B, `showlog` prints a line like `INFO learnwhile host starting`.
- [ ] The database and log are where the README says: `ls "$XDG_DATA_HOME"/learnwhile/` shows `learnwhile.db`, and `ls "$XDG_STATE_HOME"/learnwhile/` shows a `host.log.<date>` file.

## 2. A second host refuses

- [ ] With the host still running in Terminal A, run `lw host` in Terminal B.
- [ ] It refuses immediately with a message naming the socket and telling you what to do, for example: `another LearnWhile host is already listening on .../learnwhile.sock. Only one host may run at a time, so stop the other before starting this one.` Exit code is non-zero (`echo $?`).
- [ ] Terminal A is undisturbed: its pane is unchanged and the host is still running.

## 3. A killed host recovers from its stale socket

- [ ] Find and hard-kill the host: in Terminal B, `pkill -9 -f 'learnwhile host'`. (Terminal A's TUI will be left as-is; that is expected for `SIGKILL`.)
- [ ] The socket file is left behind: `ls -l "$SOCK"` still shows it.
- [ ] Start the host again in Terminal A: `lw host`. It starts cleanly, with no manual cleanup, because the stale socket is detected (a connect probe fails) and unlinked.

## 4. The log records discarded frames

- [ ] With the host running, send deliberate garbage to the socket from Terminal B: `printf 'not-a-frame\n' | nc -U "$SOCK"`. (If `nc` hangs, press Ctrl-C — the frame was already read and discarded.)
- [ ] The host keeps running: Terminal A is unaffected.
- [ ] The log says a frame was discarded and why. Give it a second (the log flushes asynchronously), then `showlog` in Terminal B shows a line like `WARN discarded trigger frame reason=Unparseable`.
- [ ] Send an unknown protocol version and see a different reason: `printf '{"v":99,"type":"trigger_open","adapter":"x","session":"s","at":"2026-01-01T00:00:00Z"}\n' | nc -U "$SOCK"`, then `showlog` shows `reason=UnknownVersion(99)`.
- [ ] A valid frame still works afterward: `printf '{"session_id":"s"}' | lw hook --open` surfaces `front1` in Terminal A. The accept loop survived the garbage.

## 5. The terminal is restored on a signal

The signal handler is for signals sent *to the process* (a supervisor, a `kill`). Note that inside
the running raw-mode TUI, Ctrl-C is delivered as a key, not as SIGINT, so it exits through the
Ctrl-C quit key — which also restores the terminal. To exercise the actual signal handler, send the
signal from Terminal B.

- [ ] Start `lw host` in Terminal A (it is in the full-screen alternate view). From Terminal B: `pkill -TERM -f 'learnwhile host'`.
- [ ] Terminal A's pane closes and the shell prompt returns cleanly. The terminal is not wrecked: typing `echo restored` echoes normally (raw mode is off) and the screen is not stuck in the alternate view. Nothing needs `reset`.
- [ ] Repeat with SIGINT: start `lw host` again, then `pkill -INT -f 'learnwhile host'` from Terminal B. Same clean restore.
- [ ] For completeness, pressing **Ctrl-C** directly in Terminal A also exits cleanly (crossterm hands it to the app as a key in raw mode, and the quit key handles it). The terminal is restored either way.

## 6. Log growth is bounded

- [ ] The log filename carries a date suffix (`host.log.2026-07-31`), which is daily rotation. A long-lived host writes a new file each day rather than one unbounded file, so it cannot fill the disk over weeks. (A full day of running is out of scope for a by-hand check; the rotating filename is the evidence.)

## Reset

```sh
rm -rf /tmp/lw-m5 && mkdir -p /tmp/lw-m5/run
```

## Gotchas

- **Both terminals need the Setup block**, especially `XDG_RUNTIME_DIR`, or Terminal B's probes will hit a different socket than Terminal A's host.
- **The log flushes asynchronously.** A discarded-frame line can take a second to appear. That buffering is what keeps logging off the hot path.
- **`nc -U` is the unix-socket mode of netcat**, which ships with macOS. If yours lacks `-U`, `socat - UNIX-CONNECT:"$SOCK"` does the same.
- **`SIGKILL` (`-9`) is the one signal that does not restore the terminal** — by definition it cannot run any cleanup. That is exactly the stale-socket case section 3 exercises; use Ctrl-C or `SIGTERM` for the clean-restore test.
