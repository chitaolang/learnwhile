# LearnWhile（繁體中文）

*[English](./README.md) | 繁體中文*

> 本文件是 [英文版 README](./README.md) 的翻譯。專案的術語定義以英文版與 [`CONTEXT.md`](./CONTEXT.md) 的詞彙表為準；
> 兩者若有出入，以英文版為準。載有特定意義的術語會在括號中附上英文原文。

一套終端機原生的間隔重複（spaced repetition）系統，把你等待 AI 編碼代理（AI coding agent）的
時間，變成一段一段簡短的複習。

當你把工作交給代理時，卡片會出現在旁邊的窗格裡；當代理需要你回來時，卡片就會清掉。這個窗格
永遠不會搶走焦點，也永遠不會擋住你：如果 LearnWhile 沒有在執行，你的代理行為就和平常完全一樣。

**目前狀態：v1 功能完成（M1–M5），另有兩項 v1 之後的新增（M6–M7）。** 你可以從檔案匯入一副牌，
並在等待的空檔做真正的複習（Review）：看題目、翻答案、評分。每一次評分都會被持久化，卡片也會由
FSRS 重新排程。在 v1 之外，日文卡片可以帶 furigana 讀音，翻答案時顯示在漢字上方（M6）；另有一個
可選擇啟用的 Prompt Gate，能在你完成一次複習之前先壓住你的下一個 prompt（M7）。後面的章節會逐一
介紹各項功能；完整歷程詳見 [`docs/milestones/`](./docs/milestones/README.md)。

## 安裝

需要 Rust 以及類 Unix 作業系統。Windows 不在 v1 範圍內 —— 傳輸層使用的是 unix domain
socket（[ADR-0004](./docs/adr/0004-unix-socket-ipc-fail-open.md)）。

```sh
cargo build --release
# 把 target/release/learnwhile 放到 PATH 上的任一個目錄
```

或者讓安裝腳本替你建置，並把執行檔複製到 PATH 上的某個 bin 目錄（預設是 `~/.local/bin`，
也可以設定 `PREFIX`）：

```sh
./scripts/install.sh
# PREFIX=/usr/local/bin ./scripts/install.sh   # 系統層級的位置，可能需要 sudo
```

之後要移除，執行 `./scripts/uninstall.sh`。加上 `--purge` 會連同你的卡片、複習歷史、log
與 socket 一併刪除。

### 或者讓 AI 代理替你安裝

LearnWhile 本來就是給已經在用 AI 編碼代理的人用的，所以最快的安裝方式，就是把整件事交給那個代理。
在這個 repo 的 clone 目錄下，把下面這段 prompt 貼給 Claude Code（或你慣用的代理）：

> 請幫我從這個 repository 安裝 LearnWhile：
> 1. 執行 `./scripts/install.sh`，建置 release 執行檔並放到我的 PATH 上。
> 2. 把 LearnWhile hook 加進我的 `~/.claude/settings.json`，綁定 `UserPromptSubmit`、`Stop` 與
>    `Notification` 三個事件，每個都執行 `learnwhile hook` 指令。請合併進我既有的 hooks、不要覆蓋，
>    並在存檔前把 diff 給我看。
> 3. 匯入 N5 牌組：`learnwhile seed data/anki-jlpt/n5.tsv`。
>
> 不要自己啟動 host，它是一個全螢幕 TUI，我會自己在我的窗格裡執行。完成後，提醒我重新啟動這個
> session，並在代理旁邊執行 `learnwhile`。

幾件要注意的事：

- **檢查 `settings.json` 的變更。** 代理正在改你全域的代理設定；接受前先看一下 diff，確認它是合併
  進你既有的 hooks，而不是取代它們。
- **加完 hooks 後重新啟動代理 session**，Claude Code 才會載入它們。
- **host 由你啟動，不是代理。** `learnwhile` 是互動式全螢幕窗格，請自己在代理旁邊的分割視窗執行
  （見下方「執行」一節）。代理無法替你操作它。
- 想改用 Prompt Gate？叫代理把 `UserPromptSubmit` 換成 `learnwhile hook --gate`（見下方
  「Prompt Gate（選用）」一節）。

## 指令總覽

`learnwhile` 是一個帶有數個子指令的執行檔。每個子指令在下面的章節都有更完整的說明。

| 指令 | 作用 |
|---|---|
| `learnwhile`<br>`learnwhile host` | 啟動複習窗格（長駐的 host）。 |
| `learnwhile hook` | Claude Code 的 hook 轉接器：從標準輸入讀取事件，接著開啟或關閉一個 Trigger。永遠以結束碼 0 收工。 |
| `learnwhile hook --open` | 強制開啟一個 Trigger，忽略事件名稱。 |
| `learnwhile hook --close` | 強制關閉一個 Trigger，忽略事件名稱。 |
| `learnwhile hook --gate` | 開啟 Prompt Gate 的 hook 轉接器：壓住你的下一個 prompt，直到你完成一次複習。 |
| `learnwhile seed <file.tsv>` | 從以 tab 分隔的檔案匯入卡片。重複執行會略過已存在的卡片。 |
| `learnwhile extract <notes.csv> [out-dir]` | 從 anki-jlpt-decks 的原始匯出檔建立 JLPT 匯入牌組。`out-dir` 預設為當前目錄。 |
| `learnwhile config` | 列出每一項設定及其值。 |
| `learnwhile config set <key> <value>` | 修改一項設定。會拒絕未知的鍵或無法使用的值。 |
| `learnwhile cards` | 列出每一張卡片，以及它的排程狀況。 |

沒有帶子指令時，`learnwhile` 會啟動 host。帶了未知的子指令時，會印出用法並以非 0 結束碼結束：

```sh
$ learnwhile wat
learnwhile: unknown subcommand "wat"
usage: learnwhile [host|hook|seed|config|cards|extract]
```

## 設定 Claude Code hook

把下面這段加進 `~/.claude/settings.json`。當你把工作交出去時，觸發會開啟；當代理完成這一輪時，
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
| `Stop` | 關閉 —— 代理完成了整輪回應（唯一的關閉時機） |
| `Notification` | 忽略 —— 它會在回合中途因為權限詢問或閒置等待而觸發，並不是你這次等待的結束 |
| 其他事件 | 忽略 —— 那不是交接的分界點 |

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

### 日文卡片的 furigana

卡片文字可以帶 Anki 風格的 furigana：`勉強[べんきょう]`，方括號裡的假名是緊接在它前面那串漢字的
讀音，需要界定範圍時用空白分隔（`この 間[あいだ]`）。讀音在問題面會被藏起來，翻答案時才堆疊顯示
在它的漢字上方，所以「讀出這個漢字」的卡片仍然是個誠實的測驗。沒有方括號的卡片，顯示方式和以前
完全一樣。詳見 [`docs/specs/furigana-ruby-display.md`](./docs/specs/furigana-ruby-display.md)。

### JLPT 牌組

現成的日文牌組放在 [`data/anki-jlpt/`](./data/anki-jlpt/)：每個 JLPT 級別各一個匯入檔（`n5.tsv`
到 `n1.tsv`，總共約 10,600 張卡），都已採用上面的 furigana 記法。匯入某一級的方式，和匯入其他
TSV 完全相同：

```sh
# 匯入 N5 牌組（807 張卡）；想從最輕鬆的一組開始就從這裡起步
learnwhile seed data/anki-jlpt/n5.tsv
# → 807 added, 0 skipped (already present)

# 想加就隨時再疊上其他級別；重複執行是安全的，重複的卡片會被略過
learnwhile seed data/anki-jlpt/n4.tsv
learnwhile seed data/anki-jlpt/n1.tsv

# 確認有匯入進去
learnwhile cards
```

匯入一整級並不會把你淹沒：`new_cards_per_day` 上限（預設為 20）仍然會控制窗格每天引入多少張新卡，
所以即使是 4,044 張卡的 N1 牌組，也是慢慢地一點一點餵給你。

這些檔案是由 `extract` 子指令，從 [`5mdld/anki-jlpt-decks`](https://github.com/5mdld/anki-jlpt-decks)
的原始匯出檔產生的：

```sh
learnwhile extract notes.csv [out-dir]   # 產生 n1.tsv .. n5.tsv；out-dir 預設為當前目錄
```

它會讀取牌組以 tab 分隔的原始檔，為每個級別各寫出一個 `front<TAB>back` 檔案，可直接用於 `seed`。
原始牌組由 **egg rolls** 製作，採用 [CC BY-NC 4.0](https://creativecommons.org/licenses/by-nc/4.0/)
授權，這些衍生牌組也沿用同一份授權（須署名 egg rolls，僅限非商業使用）。完整署名見
[`data/anki-jlpt/README.md`](./data/anki-jlpt/README.md)。

## 檢視與調整

`config` 與 `cards` 會讀取同一個資料庫，host 有沒有在執行都可以跑。`config` 列出、`config set`
修改的設定項目如下：

| 鍵 | 預設值 | 作用 |
|---|---|---|
| `trigger_expiry_seconds` | `1800` | 一個 Trigger 在遺失 close 後過期之前會維持開啟多久 |
| `desired_retention` | `0.9` | FSRS 的目標記憶保留率 |
| `new_cards_per_day` | `20` | 每日新卡引入的上限 |

Host 在啟動時讀取 config，所以改完之後要重啟才會生效。`config set` 會拒絕未知的鍵，或 host
無法使用的值，因此打錯字會在這裡就失敗，而不是在下次啟動時才出錯。

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

## 窗格會顯示什麼

每一次等待，LearnWhile 會依固定順序挑選要顯示什麼：

1. 你在這次 sitting 稍早失敗（評為 Again）的卡片，再給你一次重試的機會。
2. 否則，一張真正到期的卡片。
3. 否則，一張新卡 —— 但有每日上限，所以再忙的一天也不會把整副牌一次倒給你。
4. 否則，一個閒置窗格，顯示有幾張到期、今天還剩幾張新卡，以及下一張卡片何時到期。

卡片永遠不會在到期日之前出現，這樣你的 FSRS 間隔才會保持誠實。唯一的例外是你在這次 sitting
剛剛失敗的卡片：它會在同一天回來讓你再試一次，直到你把它評為 Again 以外的評分為止。

如果代理在你複習到一半時回來了，卡片不會遺失。它會保持原狀等著你 —— 已經翻開的答案仍然是
翻開的 —— 在你下一次等待時繼續。忽略一張卡片不會有任何代價：沒有計時器，也不會一直催你。
一次 sitting 會持續到 host 停止為止；重新啟動 host 就是重新開始。

## Prompt Gate（選用）

預設情況下，什麼都不會被擋。如果你想要一個「承諾裝置」，把你的 `UserPromptSubmit` hook 改成指向
`learnwhile hook --gate`：

```json
"UserPromptSubmit": [
  { "hooks": [{ "type": "command", "command": "learnwhile hook --gate" }] }
]
```

開啟 gate 後，你的下一個 prompt 會被壓住，直到你完成一次複習。欠著的那張卡片即使在閒置時也會留在
窗格裡，所以你隨時都能清掉它：給它評分，你的 prompt 就會通過。沒有這個 flag 時，hook 的行為不變；
而且當 LearnWhile 沒在執行、回應太慢、或沒有東西可複習時，gate 絕不會擋你。它是一個自我施加的
提醒，不是鎖。詳見 [`docs/specs/prompt-gate.md`](./docs/specs/prompt-gate.md)。

## 如果它沒有在執行會怎樣

不會怎樣。hook 會去連一個根本不存在的 socket，立刻放棄，然後以結束碼 0 收工。當掉、卡住或
根本沒啟動的主機，都不可能拖慢你的代理 —— 這正是 fail-open 測試要守住的性質，而且驗證的對象
是真正的執行檔，不是替身（mock）。

## 東西放在哪裡，以及如何重置

LearnWhile 在你的 XDG 目錄底下放三樣東西：

| 是什麼 | 放在哪裡 |
|---|---|
| 資料庫（卡片、複習歷史） | `$XDG_DATA_HOME/learnwhile/learnwhile.db`，否則 `~/.local/share/learnwhile/` |
| Log 檔（每日輪替） | `$XDG_STATE_HOME/learnwhile/host.log.<日期>`，否則 `~/.local/state/learnwhile/` |
| Socket | `$XDG_RUNTIME_DIR/learnwhile.sock`，否則 `/tmp/learnwhile.sock` |

Host 是資料庫與 socket 的唯一擁有者（[ADR-0003](./docs/adr/0003-long-lived-host-thin-adapters.md)）。
啟動第二個 host 時，它會拒絕，並顯示一段指出正在執行中的那個 host 的訊息，而不是讓兩個窗格
各說各話。如果某個 host 被強制關閉、留下了 stale socket，下次啟動會自動回復，不需要手動清理。

當有東西出問題時，log 就是該去看的地方：每一個被丟棄的 frame（連同原因）以及任何 producer
執行緒的失敗都會記在那裡。窗格刻意保持被動且安靜（[ADR-0001](./docs/adr/0001-agent-hook-trigger-passive-surface.md)），
所以承載診斷訊息的是 log，而不是畫面。

要重置，先停掉 host，再刪掉資料庫。這會清掉你的卡片、複習歷史，以及任何 config 變更。下次執行
`learnwhile seed` 或啟動 host 時，會用預設值重新建立一個空的資料庫。不論 `$XDG_DATA_HOME` 有沒有
設定，下面這個指令都適用：

```sh
rm -f "${XDG_DATA_HOME:-$HOME/.local/share}/learnwhile/learnwhile.db"
```

## 開發

```sh
cargo test      # 主機邊界測試，加上 fail-open 的子行程測試
cargo clippy --all-targets
cargo fmt
```

## 文件

- [`CONTEXT.md`](./CONTEXT.md) —— 詞彙表。這裡的術語都是有承載意義的。
- [`docs/adr/`](./docs/adr/README.md) —— 架構決策，以及每個決策付出的代價。
- [`docs/specs/`](./docs/specs/v1-trigger-spine-and-learning-engine.md) —— v1 規格。
- [`docs/milestones/`](./docs/milestones/README.md) —— 通往 v1 的五個里程碑。
