# 架構決策紀錄

每個決策一個檔案，依決策發生順序編號。標題寫成斷言，因此下方清單也可以讀成 LearnWhile 已承諾採用內容的摘要。每筆紀錄都遵循相同的三段式結構：**背景**（迫使我們做出選擇的張力）、**決策**（選了什麼、拒絕了什麼）、**後果**（這個選擇的代價）。

詞彙沿用 [`/CONTEXT.md`](../../../CONTEXT.md) 的 glossary。

## 紀錄

| # | 決策 |
|---|---|
| [0001](./0001-agent-hook-trigger-passive-surface.md) | 觸發由 AI agent 生命週期 hook 驅動，並以被動方式呈現 |
| [0002](./0002-card-selection-protects-scheduler.md) | 卡片選擇絕不提前複習；它保護排程器 |
| [0003](./0003-long-lived-host-thin-adapters.md) | LearnWhile 以長駐程序執行；adapter 是輕量 IPC client |
| [0004](./0004-unix-socket-ipc-fail-open.md) | Adapter 透過 Unix socket 連到 host；架構上 fail-open |
| [0005](./0005-runtime-open-trigger-set.md) | Runtime 追蹤 open Trigger 集合；「等待中」代表集合非空 |
| [0006](./0006-trigger-expiry-drains-phantom-opens.md) | Open Trigger 會過期，因此遺失 close 不會讓卡片永遠卡住 |
| [0007](./0007-ndjson-trigger-frames.md) | Adapter 傳送以 adapter 與 session 識別的 newline-delimited JSON frame |
| [0008](./0008-single-binary-subcommands.md) | 同一個 binary 同時作為長駐 host 與 hook client |
| [0011](./0011-session-is-host-process-lifetime.md) | 一個 Session 就是 host process 的生命週期 |

## 它們的關係

有幾筆紀錄刻意把某些決策留給後續紀錄補上，所以單讀其中一筆時，可能會高估當時已經決定的範圍：

- **0003** 延後決定 IPC transport -> **0004** 選擇 Unix socket -> **0004** 又延後決定 message format -> **0007** 定義 frame。
- **0005** 指出遺失 Trigger close 必須可被容忍，但沒有說如何做到 -> **0006** 設定 expiry policy。
- **0010** 讓 lapse queue「隨 Session 一起消滅」，但沒有說是什麼界定了一個 Session -> **0011** 把 Session 定義為 host process 的生命週期。
- **0003** 建立 thin adapter 的方向，但沒有說如何發佈 -> **0008** 把 adapter 做成 host binary 的 subcommand。

## 新增紀錄

取下一個編號，把檔名命名為 `NNNN-kebab-case-assertion.md`，並在上方新增一列。請寫明你拒絕了哪些替代方案以及原因。這通常是未來讀者最需要的部分，因為被拒絕的選項，往往正是他們準備再次提出的選項。
