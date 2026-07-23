# M1 — Trigger spine

**Goal.** A developer can run LearnWhile in a pane beside Claude Code and watch a card appear
the moment they submit a prompt and clear the moment the agent needs them back.

## Demo

1. Build, and add the `hook` subcommand to `~/.claude/settings.json` on `UserPromptSubmit`,
   `Stop`, `PermissionRequest`, and `Elicitation`.
2. Run `learnwhile` in a second pane.
3. Submit a prompt to Claude Code. A card appears in the LearnWhile pane.
4. Wait for the agent to finish, or for it to ask permission. The card clears.
5. Kill the LearnWhile process and submit another prompt. Claude Code behaves exactly as
   before — no error, no delay.

Step 5 is the important one. It is the fail-open claim, and it is the claim that most needs to
be true before anything else is built on top.

## What ships

**UI.** A `ratatui` pane with two states: a hardcoded card while Waiting, and a placeholder idle
state otherwise. A quit key that restores the terminal cleanly. Nothing is interactive yet —
there is no reveal and no rating, because there is no card to rate.

**Backend.** The `hook` and host subcommands; the unix socket and NDJSON frames; the event loop
and its producer threads; the open-Trigger set and its expiry sweep.

## Sub-tasks

1. **Stand up the crate.** `Cargo.toml`, `src/main.rs`, binary named `learnwhile`. Dispatch
   subcommands by matching on `std::env::args()` rather than adding `clap` — there are three
   subcommands with no flags, and ADR-0008 makes argument parsing part of the hook's cold path.
   Revisit if the CLI grows options.
2. **Frame types.** `TriggerFrame { v, type, adapter, session, at }` with `serde`, per ADR-0007.
   Serialize on the hook side, deserialize on the host side, one JSON object per line.
3. **Socket path resolution.** `$XDG_RUNTIME_DIR/learnwhile.sock`, with a documented fallback
   when the variable is unset. Shared by both subcommands; it is the *only* thing the hook path
   is permitted to resolve (ADR-0008).
4. **The `hook` subcommand.** Read Claude Code's hook JSON from stdin, extract the session id and
   the event name, map it to `trigger_open` or `trigger_close`, connect, set a write timeout,
   write one frame, exit 0. Every failure path — no socket, refused, timeout, malformed stdin,
   panic — also exits 0. No SQLite, no TUI, no logging.
5. **Host listener.** Bind the socket, unlinking a stale file first. Read line-at-a-time under a
   bounded maximum line length; a line that doesn't parse, or carries an unknown `v`, is dropped
   without killing the accept loop (ADR-0007).
6. **Event loop.** The `Event` enum and one `mpsc` channel fed by three producer threads —
   accept, terminal input, sweep tick — per ADR-0009. The main thread owns all state.
7. **Open-Trigger set.** Keyed by `(adapter, session)`. Open inserts, close removes; a duplicate
   open is idempotent and a close for an unknown key is ignored. Waiting is the set being
   non-empty; surface on the empty→non-empty edge, clear on the reverse (ADR-0005).
8. **Expiry sweep.** Each entry carries an expiry measured from open, draining on the tick thread
   (ADR-0006). M1 has no database, so the 1800-second default is a constant here; M2 moves it
   into `config`, which is where ADR-0006 requires it to live.
9. **Renderer.** Draw the hardcoded card while Waiting, the placeholder otherwise. The pane must
   never take foreground focus (ADR-0001).
10. **`spawn_test_host`.** The harness the rest of the project depends on: boots the host
    in-process with a temp socket path, a controllable clock, and a `TestBackend`, and hands back
    a `Sender` clone for injecting key events. Worth more care than anything else in this
    milestone — it sets the testing pattern for the whole repo.
11. **Install notes.** The `settings.json` hook snippet, in the README.

## Tests

Through the host boundary, driving the **real socket** with the **same frames the hook writes**:

- An open surfaces a card; a close clears it.
- A duplicate open is idempotent; a close for an unknown key changes nothing.
- Two overlapping Triggers keep the card up until both close — the ADR-0005 case.
- A Trigger whose close never arrives expires on the sweep and clears the card, with the clock
  advanced explicitly rather than by sleeping.
- A malformed line, an oversized line, and an unknown `v` are each ignored, and the host still
  accepts a valid frame afterwards.
- The quit key exits and restores the terminal.

As subprocess tests, because "the real binary exits 0" is precisely the claim:

- `hook` with no host running exits 0 within its timeout.
- `hook` against a socket file whose listener is gone exits 0.
- `hook` against a deliberately wedged socket — bound, accepting, never reading — exits 0.

## Exit criteria

- The demo works end to end against real Claude Code, not just against the test harness.
- Killing the host mid-session leaves Claude Code unaffected.
- No test reaches into the open-Trigger set directly.

## Not in this milestone

- Any real card, deck, or database — **M2**.
- Reveal and rating keys — **M2**.
- Card selection of any kind — **M3**.
- Session state and lapses — **M4**.
- Logging, single-instance detection, packaging — **M5**.

## Decisions this relies on

ADR-0001 (passive surface), ADR-0003 (long-lived host, thin adapters), ADR-0004 (unix socket,
fail-open), ADR-0005 (open-Trigger set), ADR-0006 (expiry), ADR-0007 (NDJSON frames), ADR-0008
(single binary), ADR-0009 (event loop).

## Risks

**The hook events may not fire where we assume.** ADR-0001 maps a close onto the first of `Stop`,
`PermissionRequest`, or `Elicitation`, but that mapping has not been observed in practice. This
is the single riskiest assumption in v1 and the reason this milestone is first. Verify it by
watching real Triggers before building anything on top; if the mapping is wrong, the fix is
cheap here and expensive later.

**`UnixStream::connect` has no timeout parameter in std.** A refused connect returns immediately
and a wedged host fails at the write, so a write timeout should be sufficient — but this is an
argument, not a measurement. The wedged-socket test is what turns it into a measurement, so write
that test early rather than last.
