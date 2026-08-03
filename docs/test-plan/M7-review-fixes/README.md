# M7 review-fix verification — Prompt Gate

A short by-hand checklist confirming the code-review fixes on top of M7. The fixes are
behavior-preserving except for one thing you can see: the gate **verdict now travels as a JSON
frame on the wire** (ADR-0007), not a bare `allow`/`block` token.

Full gate behavior is dogfooded in the [M7 plan](../M7/README.md); re-run that for the behavioral
regression. This plan covers only what the fixes touched.

## What the fixes changed

- **JSON gate verdict (ADR-0007).** The host's reply to a gate query is now
  `{"v":1,"type":"gate_verdict","verdict":"allow"|"block"}`, fail-open to `allow` on any garble.
- **Test-only / refactor (no behavior change).** Added latency, no-round-trip, and exhausted-deck
  tests; deduped the gate client and the host's open-and-surface path; bundled the gate state into a
  `Gate` type. Nothing here is user-visible, so this plan does not re-test it beyond the smoke check.

## Setup

Same isolated setup as the other plans, in **every terminal**:

```sh
cd /Users/timlin/learnwhile
cargo build --release
alias lw="$PWD/target/release/learnwhile"
export XDG_DATA_HOME=/tmp/lw-test              # reset with: rm -rf /tmp/lw-test
```

Terminal B helpers (one session `s`):

```sh
lwgate()  { printf '{"session_id":"s"}' | lw hook --gate --open; echo " (exit=$?)"; }
lwopen()  { printf '{"session_id":"s"}' | lw hook --open; }
lwclose() { printf '{"session_id":"s"}' | lw hook --close; }
```

- [ ] Fresh binary: `strings "$PWD/target/release/learnwhile" | grep -c trigger_expiry_seconds` is non-zero.
- [ ] Seed a deck: `printf 'Capital of France\tParis\nBorrow checker\tEnforces ownership at compile time\n' > /tmp/deck.tsv && lw seed /tmp/deck.tsv`

## 1. The gate still allows, blocks, and pays (smoke)

Confirms the JSON-verdict change did not break the allow/block/pay flow.

- [ ] Terminal A: `lw host` shows `Not waiting`.
- [ ] Terminal B: `lwgate`. Prints ` (exit=0)` — allowed. Terminal A shows the first card.
- [ ] Do not review. `lwclose`. Terminal A holds the card with heading `Review to continue`.
- [ ] `lwgate`. Prints `{"decision":"block","reason":"Finish one review to continue."} (exit=0)` —
  blocked. This hook-to-Claude output is unchanged by the fixes.
- [ ] Terminal A: press **space**, then **3**. The pane returns to `Not waiting`.
- [ ] `lwgate`. Allowed again.

## 2. The verdict is a JSON frame on the wire (ADR-0007 fix)

Peek at the raw reply the host sends back, which the automated tests exercise but no one sees by
hand. Do this **while a review is owed** (so the reply is `block` and dialing the socket has no side
effect — a blocked query never opens a Trigger):

- [ ] Get into the owed state: `lwgate` (allowed, card up), then `lwclose` (Terminal A shows
  `Review to continue`).
- [ ] Dial the socket by hand and send a gate query:

  ```sh
  printf '{"v":1,"type":"gate_query","adapter":"claude-code","session":"probe","at":"2026-01-01T00:00:00Z"}\n' \
    | nc -U /tmp/learnwhile.sock
  ```
- [ ] The reply line is **JSON**, not a bare token:
  `{"v":1,"type":"gate_verdict","verdict":"block"}`. Press **Ctrl-C** once you see it (the host keeps
  the connection open for more frames).
- [ ] It is **not** the old bare `block`.

Optional — see the `allow` variant from a clean state (this one *does* open a probe Trigger, so
`lwclose`-style cleanup with session `probe` after): pay the owed review first (space, 3), then run
the same `nc` command. The reply is `{"v":1,"type":"gate_verdict","verdict":"allow"}`.

If `nc` lacks `-U`, use socat:
`printf '...' | socat - UNIX-CONNECT:/tmp/learnwhile.sock`.

## 3. Gate-off and fail-open are unchanged

- [ ] Un-gated handoff: `lwopen` surfaces a card and `lwclose` clears it; no prompt is ever blocked
  and the idle pane shows no held card. (The un-gated hook fire-and-forgets, making no round-trip.)
- [ ] Quit the host (**q** in Terminal A), then `lwgate`. Returns instantly with ` (exit=0)` and no
  block. With no host, the gate fails open.

## Reset

```sh
rm -rf /tmp/lw-test
```

## Gotchas

- **Two different messages.** Step 2 shows the **host to hook** reply (`gate_verdict`, the new JSON
  wire form). The **hook to Claude Code** output on a block is a different message,
  `{"decision":"block",...}`, and is unchanged (step 1). Don't conflate them.
- **The verdict fails open.** If the host ever sends an unparseable or wrong-version reply, the hook
  reads it as `allow`. You will not reproduce that by hand, but it is why a garbled verdict never
  blocks you.
- **`nc -U` holds the connection.** It prints the reply then waits; Ctrl-C to exit.
- **Debt is Session-wide, not per session id.** The `probe` session id in step 2 is arbitrary; the
  verdict reflects the host's single review debt regardless of which session asks.
