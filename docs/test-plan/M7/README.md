# M7 manual test plan — Prompt Gate

A by-hand checklist for the [M7 milestone](../../milestones/README.md#m7-prompt-gate): opt into the Prompt
Gate and confirm that an owed Review holds the next prompt, that the owed card is payable from the
idle pane, that completing a Review lets the prompt through, and that with the gate off (or no host)
nothing ever blocks.

The automated suite (`cargo test`, `tests/gate.rs`) already covers all of this. This plan is for
dogfooding against the real binary, on a real terminal, where the host's TUI and the hook's exit
code are both in play.

## How this plan drives the gate

In real use you enable the gate by pointing your `UserPromptSubmit` hook at `learnwhile hook
--gate` in `settings.json`, and Claude Code fires it on every prompt. This plan needs no live agent:
it simulates a gated handoff by hand with `learnwhile hook --gate --open`, which performs the **same**
gate round-trip and prints the **same** verdict. On a block the hook prints the block JSON to stdout
(what Claude Code would act on) and exits 0; on allow it prints nothing and exits 0.

## Setup

Do this once, in **every terminal** you use. The plan uses an isolated data directory so your real
deck is never touched, and starts with a build so you never test a stale binary.

```sh
cd /Users/timlin/learnwhile
cargo build --release
alias lw="$PWD/target/release/learnwhile"     # define in each terminal
export XDG_DATA_HOME=/tmp/lw-test              # isolate the deck; reset with: rm -rf /tmp/lw-test
```

You will use two terminals:

- **Terminal A** runs the host (a full-screen TUI).
- **Terminal B** simulates handoffs and inspects the deck.

Both need the alias and `XDG_DATA_HOME`. The socket is shared automatically at
`/tmp/learnwhile.sock` (your `XDG_RUNTIME_DIR` is unset). Define these helpers in **Terminal B**
(all use one session `s`, so opens and closes pair up):

```sh
lwgate()  { printf '{"session_id":"s"}' | lw hook --gate --open; echo " (exit=$?)"; }
lwopen()  { printf '{"session_id":"s"}' | lw hook --open; }        # un-gated handoff
lwclose() { printf '{"session_id":"s"}' | lw hook --close; }        # the agent returns
```

`lwgate` prints the verdict then `(exit=N)`: an allowed prompt shows just ` (exit=0)`, a blocked one
shows `{"decision":"block","reason":"Finish one review to continue."} (exit=0)`.

- [ ] The binary is fresh: `strings "$PWD/target/release/learnwhile" | grep -c trigger_expiry_seconds` prints a non-zero number.
- [ ] Seed a small deck:

  ```sh
  printf 'Capital of France\tParis\nRust ownership rule\tEach value has exactly one owner\nBorrow checker\tEnforces ownership at compile time\n' > /tmp/deck.tsv
  lw seed /tmp/deck.tsv        # 3 added, 0 skipped
  ```

## 1. Gate off behaves exactly like v1

- [ ] Terminal A: `lw host` shows `Not waiting`.
- [ ] Terminal B: `lwopen`. Terminal A flips to `Waiting` and shows the first card.
- [ ] `lwclose`. Terminal A returns to `Not waiting` and the card is gone. No prompt is ever blocked,
  and the idle pane shows no card. Nothing about the un-gated path changed.

## 2. The first gated prompt is allowed and surfaces a card

- [ ] Terminal B: `lwgate`. It prints just ` (exit=0)` — allowed.
- [ ] Terminal A flips to `Waiting` and shows the first card's front. The gate opened the Trigger
  itself on allow.

## 3. An owed Review holds the next prompt

- [ ] Do **not** review. Terminal B: `lwclose` (the agent returns).
- [ ] Terminal A keeps the card on screen with the heading **`Review to continue`**, even though you
  are idle. The owed card is held so the debt is payable.
- [ ] Terminal B: `lwgate` again. This time it prints
  `{"decision":"block","reason":"Finish one review to continue."} (exit=0)` — **blocked**.
- [ ] Terminal A still shows the owed card. The gate did not open a new wait.

## 4. Pay the debt from the idle pane, then the prompt goes through

- [ ] Terminal A, with the owed card up (`Review to continue`): press **space**. The back appears.
- [ ] Press **3** (Good). Terminal A returns to `Not waiting`; the card is gone.
- [ ] Terminal B: `lwgate`. It prints ` (exit=0)` — allowed — and Terminal A surfaces the next card.
  The debt was cleared from the idle pane.

## 5. Reviewing during the wait also clears the debt

- [ ] With a card up from step 4 (Waiting), review it in place: **space**, then **3**.
- [ ] Terminal B: `lwclose`, then `lwgate`. Allowed (` (exit=0)`). One Review this cycle was enough;
  you did not have to wait until idle.

## 6. The gate allows when nothing is reviewable

- [ ] Finish the deck: review any remaining cards (`space` then a rating each) until Terminal A shows
  `Not waiting` with `Due now: 0` and `New remaining: 0` even after an `lwgate`.
- [ ] Terminal B: `lwgate`. Allowed (` (exit=0)`), and Terminal A stays idle. With no card to surface,
  no debt is incurred, so the gate cannot block.

## 7. Fail-open: no host never blocks

- [ ] Terminal A: press **q** to quit the host.
- [ ] Terminal B: `lwgate`. It returns immediately with ` (exit=0)` and prints no block. With no host
  to answer, the gate fails open and the prompt proceeds. Quitting LearnWhile bypasses the gate by
  design.

## 8. Debt does not survive a restart

- [ ] Terminal A: `lw host` again. Terminal B: `lwgate` (allowed), do **not** review, `lwclose`.
- [ ] Terminal A: quit (**q**), then `lw host` again.
- [ ] Terminal B: `lwgate`. Allowed — the debt reset with the new host process (a Session is the host
  lifetime). The idle pane shows no held card until a new wait re-arms it.

## Reset

Start over from an empty deck at any time:

```sh
rm -rf /tmp/lw-test
```

## Gotchas

- **Real opt-in is `settings.json`.** In actual use, set the `UserPromptSubmit` hook to `learnwhile
  hook --gate`. This plan simulates that with `lw hook --gate --open`; both do the identical gate
  round-trip.
- **The block prints with no trailing newline.** That is why `lwgate` adds ` (exit=N)`. In real
  Claude Code the printed reason is shown to you and the prompt is erased, so you retype after
  reviewing.
- **The owed card shows while idle only after a gate query.** A session that never runs a gated hook
  (only `lwopen`/`lwclose`) never shows a card on the idle pane — v1 behavior is untouched.
- **The gate needs a live host, and the host is a TUI.** Run the host in a real terminal; a
  non-tty host exits at raw-mode setup and cannot answer a gate query.
- **The gate is only as present as the host.** Removing `--gate`, or quitting the host, always lets
  prompts through. The gate is a self-imposed commitment device, not a lock.
