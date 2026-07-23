# M5 — Hardening and install

**Goal.** A developer can install LearnWhile, leave it running every day, and work out what
happened when something misbehaves — without reading the source.

## Demo

1. Follow the README from a clean machine: build, install the hook, seed a deck, run the host.
2. Start a second host. It refuses with a clear message naming the running one, rather than two
   panes silently disagreeing.
3. Kill the host with `SIGKILL`, leaving a stale socket file behind. Start it again. It starts
   cleanly with no manual cleanup.
4. Send deliberate garbage to the socket. The host keeps running, and the log file says a frame
   was discarded and why.
5. Leave it running through a full working day. It is still there, still correct, and the log has
   not filled the disk.

## What ships

**UI.** The failure-facing surface: a clear refusal when a second host starts, a clean terminal
restore on quit and on signal, and a README a developer can actually follow.

**Backend.** Logging to a file, single-instance detection, stale-socket recovery, and the hook
latency budget turned from an intention into an assertion.

## Sub-tasks

1. **Log file.** `tracing` with a file subscriber under the XDG state directory. ADR-0007 makes
   this mandatory rather than nice-to-have: the host silently ignores malformed frames to stay
   fail-open, which makes adapter bugs invisible unless something records them. The pane cannot
   serve this purpose — it must stay passive (ADR-0001).
2. **Log what was discarded.** Every dropped frame logs its reason: unparseable, unknown `v`,
   oversized line. Also log producer-thread death, which ADR-0009 flags as a way to silently
   starve an input.
3. **Bounded log growth.** A daily rotation or a size cap. A long-lived process that writes an
   unbounded file is a bug that takes weeks to show up and then fills a disk.
4. **Single-instance detection.** One host owns the socket and the database (ADR-0003). Detect a
   live listener on the socket and refuse to start with a message that says what is running and
   what to do, rather than a bind error.
5. **Stale socket recovery.** Distinguish a stale socket file from a live one — attempt a connect
   before unlinking, so recovery never unlinks a socket a running host is using. This and the
   previous sub-task are the same code path from opposite directions; build them together.
6. **Hook latency assertion.** ADR-0008 says keeping the hook path minimal is worth asserting in
   a test that measures it, not just intending. Add that test, with a budget generous enough not
   to flake on CI but tight enough to catch someone opening SQLite on the hook path.
7. **Terminal restore on signal.** `SIGINT` and `SIGTERM` restore the terminal, not just the quit
   key. A tool that leaves a wrecked terminal after `Ctrl-C` gets uninstalled.
8. **README.** Install, the `settings.json` hook snippet, seeding, running, where the database and
   log live, and how to reset. Enough that the M5 demo needs nothing else.
9. **Release profile.** Whatever makes the binary reasonable to install locally. Binary size is
   explicitly not a concern (ADR-0008); startup time is.

## Tests

- Starting a second host while one is running fails with the expected message and does not
  disturb the first — including its socket and its database.
- A stale socket file left by a killed host does not prevent startup.
- A socket file with a *live* listener is never unlinked.
- Each discarded-frame reason produces a log line, and the host continues accepting valid frames.
- The `hook` subcommand completes within its latency budget, measured on the real binary.
- The terminal is restored after `SIGINT` and `SIGTERM`.

## Exit criteria

- The M5 demo can be performed by someone who has not read this repo.
- Every fail-open path from the spec's user stories 27–31 has a test.
- No diagnosis of a misbehaving host requires attaching a debugger or adding a `println!`.

## Not in this milestone

- Windows support: out of v1 (ADR-0004; unix sockets are not native on older Windows).
- Multi-host or multi-user operation: out of v1. One host owns the socket and the database.
- Distribution beyond a local `cargo install` — no packaging, no release automation.
- The Analytics Engine. `review_history` is rich enough to feed it, but nothing reads it, which
  is worth stating plainly: shipping v1 is not the same as having answered the hypothesis.

## Decisions this relies on

ADR-0001 (the pane stays passive, so the log carries diagnostics), ADR-0003 (sole ownership),
ADR-0004 (fail-open), ADR-0007 (silent discard demands a log), ADR-0008 (hook stays cold),
ADR-0009 (producer-thread failure must be visible).

## Risks

**Hook latency tests are prone to flaking on shared CI.** Measure the binary's own runtime rather
than wall-clock including process spawn if it proves noisy, and prefer a budget that catches a
real regression — an opened database, a loaded config — over one that chases microseconds.

**Single-instance detection can race.** Two hosts starting simultaneously can both see no live
listener. The bind itself is the real serialization point, so treat the connect probe as a
courtesy that produces a better message, and let a failed bind remain the authoritative answer.
