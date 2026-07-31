# LearnWhile

A terminal-native spaced-repetition system that turns the time you spend waiting on an AI coding
agent into short bursts of review.

When you hand work to your agent, a card appears in a pane beside it. When the agent needs you
back, the card clears. The pane never steals focus, and nothing is ever blocked: if LearnWhile
is not running, your agent behaves exactly as it always has.

**Status: M1 — the Trigger spine.** Triggers, the pane, and fail-open all work end to end. The
card is a hardcoded placeholder; real cards, FSRS scheduling, and persistence arrive in M2 and
M3. See [`docs/milestones/`](./docs/milestones/README.md).

## Install

Requires Rust and a Unix-like OS. Windows is out of scope for v1 — the transport is a unix
domain socket ([ADR-0004](./docs/adr/0004-unix-socket-ipc-fail-open.md)).

```sh
cargo build --release
# put target/release/learnwhile somewhere on your PATH
```

## Wire up the Claude Code hook

Add this to `~/.claude/settings.json`. A Trigger opens when you hand off and closes when the
agent needs you back:

```json
{
  "hooks": {
    "UserPromptSubmit": [
      { "hooks": [{ "type": "command", "command": "learnwhile hook" }] }
    ],
    "Stop": [
      { "hooks": [{ "type": "command", "command": "learnwhile hook" }] }
    ],
    "Notification": [
      { "hooks": [{ "type": "command", "command": "learnwhile hook" }] }
    ]
  }
}
```

The same command is wired to every event; the adapter reads `hook_event_name` from the hook's
JSON on stdin and decides for itself:

| Claude Code event | Trigger |
|---|---|
| `UserPromptSubmit` | opens — you have handed control to the agent |
| `Stop` | closes — the agent finished its turn |
| `Notification` | closes — the agent wants permission or input |
| anything else | ignored — not a handoff boundary |

> **Note on the docs.** [ADR-0001](./docs/adr/0001-agent-hook-trigger-passive-surface.md) and the
> v1 spec name `PermissionRequest` and `Elicitation` as the closing events. Neither exists in
> Claude Code — the real events are `PreToolUse`, `PostToolUse`, `UserPromptSubmit`, `Stop`,
> `SubagentStop`, `SessionStart`, `SessionEnd`, `PreCompact`, and `Notification`, and the
> permission prompt surfaces as `Notification`. The code above is correct; those documents need
> amending to match.

If you would rather be explicit than let the adapter infer, `learnwhile hook --open` and
`learnwhile hook --close` force the transition regardless of the event name.

## Run

```sh
learnwhile          # or: learnwhile host
```

Put it in a pane beside your agent — a tmux or Zellij split, or a second terminal. LearnWhile
does not arrange your layout for you; that is your environment, not its business
([ADR-0003](./docs/adr/0003-long-lived-host-thin-adapters.md)).

Press `q` to quit.

## What happens if it is not running

Nothing. The hook connects to a socket that is not there, gives up instantly, and exits 0. A
crashed, wedged, or absent host cannot stall your agent — that is the property the fail-open
tests exist to defend, and it is checked against the real binary rather than a mock.

## Development

```sh
cargo test      # host-boundary tests plus the fail-open subprocess tests
cargo clippy --all-targets
cargo fmt
```

Tests boot the host in-process through one seam, then drive it by writing the same frames the
hook writes down a real unix socket, and assert on what the pane displays. No test reaches into
the open-Trigger set or the event loop directly.

## Documentation

- [`CONTEXT.md`](./CONTEXT.md) — the glossary. Terms here are load-bearing.
- [`docs/adr/`](./docs/adr/README.md) — architecture decisions, and what each one cost.
- [`docs/specs/`](./docs/specs/v1-trigger-spine-and-learning-engine.md) — the v1 spec.
- [`docs/milestones/`](./docs/milestones/README.md) — the five milestones to v1.

---

# LearnWhile（繁體中文）

> 本節是上方英文內容的翻譯。專案的術語定義以英文版與 [`CONTEXT.md`](./CONTEXT.md) 的詞彙表為準；
> 兩者若有出入，以英文版為準。載有特定意義的術語會在括號中附上英文原文。

一套終端機原生的間隔重複（spaced repetition）系統，把你等待 AI 編碼代理（AI coding agent）的
時間，變成一段一段簡短的複習。

當你把工作交給代理時，卡片會出現在旁邊的窗格裡；當代理需要你回來時，卡片就會清掉。這個窗格
永遠不會搶走焦點，也永遠不會擋住你：如果 LearnWhile 沒有在執行，你的代理行為就和平常完全一樣。

**目前狀態：M1 — 觸發骨幹（Trigger spine）。** 觸發（Trigger）、窗格與 fail-open 都已經可以
端到端運作。卡片目前是寫死的佔位內容；真正的卡片、FSRS 排程與資料持久化會在 M2 與 M3 完成。
詳見 [`docs/milestones/`](./docs/milestones/README.md)。

## 安裝

需要 Rust 以及類 Unix 作業系統。Windows 不在 v1 範圍內 —— 傳輸層使用的是 unix domain
socket（[ADR-0004](./docs/adr/0004-unix-socket-ipc-fail-open.md)）。

```sh
cargo build --release
# 把 target/release/learnwhile 放到 PATH 上的任一個目錄
```

## 設定 Claude Code hook

把下面這段加進 `~/.claude/settings.json`。當你把工作交出去時，觸發會開啟；當代理需要你回來時，
觸發會關閉：

```json
{
  "hooks": {
    "UserPromptSubmit": [
      { "hooks": [{ "type": "command", "command": "learnwhile hook" }] }
    ],
    "Stop": [
      { "hooks": [{ "type": "command", "command": "learnwhile hook" }] }
    ],
    "Notification": [
      { "hooks": [{ "type": "command", "command": "learnwhile hook" }] }
    ]
  }
}
```

每個事件都綁定同一個指令；轉接器（Trigger Adapter）會從 hook 由標準輸入送進來的 JSON 讀取
`hook_event_name`，自己判斷該做什麼：

| Claude Code 事件 | 觸發（Trigger） |
|---|---|
| `UserPromptSubmit` | 開啟 —— 你已經把控制權交給代理 |
| `Stop` | 關閉 —— 代理完成了這一輪 |
| `Notification` | 關閉 —— 代理需要權限或輸入 |
| 其他事件 | 忽略 —— 那不是交接的分界點 |

> **關於文件的說明。** [ADR-0001](./docs/adr/0001-agent-hook-trigger-passive-surface.md) 與 v1
> 規格把 `PermissionRequest` 和 `Elicitation` 列為關閉事件，但這兩個事件在 Claude Code 中並不
> 存在 —— 實際的事件是 `PreToolUse`、`PostToolUse`、`UserPromptSubmit`、`Stop`、`SubagentStop`、
> `SessionStart`、`SessionEnd`、`PreCompact` 與 `Notification`，而權限提示是以 `Notification`
> 的形式出現。上面的設定是正確的；需要修正的是那兩份文件。

如果你寧可明確指定、而不想讓轉接器自己推斷，可以用 `learnwhile hook --open` 和
`learnwhile hook --close` 強制指定要送出的轉換，忽略事件名稱。

## 執行

```sh
learnwhile          # 或：learnwhile host
```

把它放在代理旁邊的窗格裡 —— tmux 或 Zellij 的分割視窗，或是第二個終端機都可以。LearnWhile
不會替你安排版面配置；那是你的環境，不是它該插手的事
（[ADR-0003](./docs/adr/0003-long-lived-host-thin-adapters.md)）。

按 `q` 離開。

## 如果它沒有在執行會怎樣

不會怎樣。hook 會去連一個根本不存在的 socket，立刻放棄，然後以結束碼 0 收工。當掉、卡住或
根本沒啟動的主機，都不可能拖慢你的代理 —— 這正是 fail-open 測試要守住的性質，而且驗證的對象
是真正的執行檔，不是替身（mock）。

## 開發

```sh
cargo test      # 主機邊界測試，加上 fail-open 的子行程測試
cargo clippy --all-targets
cargo fmt
```

測試會透過單一接縫（seam）在行程內啟動主機，接著用「與 hook 寫出的完全相同的訊框（frame）」
寫進真正的 unix socket 來驅動它，並針對窗格顯示的內容做斷言。沒有任何測試會直接去碰開啟中的
觸發集合（open-Trigger set）或事件迴圈。

## 文件

- [`CONTEXT.md`](./CONTEXT.md) —— 詞彙表。這裡的術語都是有承載意義的。
- [`docs/adr/`](./docs/adr/README.md) —— 架構決策，以及每個決策付出的代價。
- [`docs/specs/`](./docs/specs/v1-trigger-spine-and-learning-engine.md) —— v1 規格。
- [`docs/milestones/`](./docs/milestones/README.md) —— 通往 v1 的五個里程碑。
