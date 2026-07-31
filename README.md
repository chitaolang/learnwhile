# LearnWhile

A terminal-native spaced-repetition system that turns the time you spend waiting on an AI coding
agent into short bursts of review.

When you hand work to your agent, a card appears in a pane beside it. When the agent needs you
back, the card clears. The pane never steals focus, and nothing is ever blocked: if LearnWhile
is not running, your agent behaves exactly as it always has.

**Status: M2 — cards and reviews.** Seed a deck from a file and do real Reviews during your
waits: see the question, reveal the answer, rate your recall. Every rating is persisted and the
card is rescheduled by FSRS. Honest due-vs-new selection and a stats pane arrive in M3. See
[`docs/milestones/`](./docs/milestones/README.md).

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

## Seed a deck

Cards come from a tab-separated file, one card per line: the front, a tab, then the back.

```
What does FSRS stand for?	Free Spaced Repetition Scheduler
Capital of France	Paris
```

Load it into your deck:

```sh
learnwhile seed cards.tsv
```

Re-running is safe. A card already in your deck is skipped, so you can edit the file and seed
again without duplicating anything. This is a convenience for trying LearnWhile, not an
Anki-style importer, so it takes TSV and nothing else. The database lives under your XDG data
directory (`$XDG_DATA_HOME/learnwhile/`, or `~/.local/share/learnwhile/`).

## Run

```sh
learnwhile          # or: learnwhile host
```

Put it in a pane beside your agent — a tmux or Zellij split, or a second terminal. LearnWhile
does not arrange your layout for you; that is your environment, not its business
([ADR-0003](./docs/adr/0003-long-lived-host-thin-adapters.md)).

When a card appears, press space to reveal the answer, then rate your recall: `1` Again, `2`
Hard, `3` Good, `4` Easy. The available keys are always shown along the bottom of the pane. Your
rating is saved the instant you press it and the card is rescheduled by FSRS. Press `q` to quit.

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

**目前狀態：M2 — 卡片與複習（cards and reviews）。** 你可以從檔案匯入一副牌，並在等待的空檔
做真正的複習（Review）：看題目、翻答案、為自己的記憶評分。每一次評分都會被持久化，卡片也會
由 FSRS 重新排程。誠實的「到期 vs 新卡」選卡邏輯與統計窗格會在 M3 完成。
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

## 匯入一副牌

卡片來自一個以 tab 分隔的檔案，每行一張卡：正面、一個 tab，然後是背面。

```
What does FSRS stand for?	Free Spaced Repetition Scheduler
Capital of France	Paris
```

把它匯入你的牌組：

```sh
learnwhile seed cards.tsv
```

重複執行是安全的。已經在牌組裡的卡片會被略過，所以你可以修改檔案後再次匯入，不會產生重複。
這只是讓你方便試用 LearnWhile 的功能，並不是 Anki 那種匯入器，因此它只吃 TSV、不吃其他格式。
資料庫存放在你的 XDG 資料目錄底下（`$XDG_DATA_HOME/learnwhile/`，或 `~/.local/share/learnwhile/`）。

## 執行

```sh
learnwhile          # 或：learnwhile host
```

把它放在代理旁邊的窗格裡 —— tmux 或 Zellij 的分割視窗，或是第二個終端機都可以。LearnWhile
不會替你安排版面配置；那是你的環境，不是它該插手的事
（[ADR-0003](./docs/adr/0003-long-lived-host-thin-adapters.md)）。

當卡片出現時，按空白鍵（space）翻出答案，接著為自己的記憶評分：`1` Again、`2` Hard、`3` Good、
`4` Easy。可用的按鍵一直顯示在窗格底部。你一按下評分，結果就會立刻儲存，卡片也會由 FSRS
重新排程。按 `q` 離開。

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
