# v1：Trigger spine + Learning Engine

詞彙沿用 [`/CONTEXT.md`](../../../CONTEXT.md)。決策以 ADR-NNNN 引用自 [`docs/adr/`](../../adr/)。

## 問題陳述

使用 AI coding agent 的開發者，一天中有很大一部分時間都在等待：agent 正在思考、執行工具，或處理多步驟修改；開發者除了看著它以外沒有事可做。這些等待很頻繁也很短：短到不足以開始真正工作，卻長到會打斷 flow。開發者於是漂到 browser tab 或 chat window，回來時比離開前更難專注。

另外，同一位開發者確實有一些想記住的東西：語言細節、不熟悉 codebase 的詞彙、framework API。Spaced repetition 是已知解法，但它要求每天刻意坐下來複習，這會與真正工作競爭，而且通常輸掉。複習習慣會死掉，不是因為開發者不想要，而是因為它從來沒有在一天裡找到位置。

這兩個問題可以互相解決，但目前沒有東西把它們接起來。既有 SRS 工具不知道開發者何時正在等待，agent 也不知道開發者想學東西。

## 解法

LearnWhile 以長駐程序形式，執行在 coding agent 旁邊的 pane 中。當開發者把工作交給 agent 時，一張卡片會出現在該 pane；當 agent 需要開發者回來時，卡片會清掉。複習發生在本來就存在的空檔中，不需要專門撥出時間。

這個 surface 刻意是被動的（ADR-0001）。它永遠不搶 focus，因此開發者可以完全忽略它，也仍然不會錯過權限提示或 agent 完成。任何東西都不會被阻塞；整個系統在架構上就是 fail-open（ADR-0004）。

排程採用真正的 FSRS，不是玩具。關鍵是，LearnWhile 絕不會只因為開發者剛好閒著，就把尚未到期的卡片提前拉出來（ADR-0002）；它會顯示到期卡，否則在每日上限內引入新卡，否則顯示中性的 idle 狀態。等待時間被利用了，但不破壞 spaced repetition 能發揮作用的間隔。

這份 spec 涵蓋完整 v1 spine：Claude Code Trigger Adapter、Runtime Engine 的 open-Trigger set、帶 FSRS 排程與 Review flow 的 Learning Engine、terminal Renderer，以及 SQLite storage。手動新增卡片不在範圍內；卡片從檔案 seed。

## 使用者故事

**等待時浮出卡片**

1. 身為開發者，我希望在向 coding agent 送出 prompt 時，LearnWhile pane 能出現一張卡片，讓我本來要浪費的等待變成複習。
2. 身為開發者，我希望 agent 需要我回來的瞬間，卡片就被清掉，讓我的注意力回到 agent，而不是停在做一半的複習。
3. 身為開發者，我希望卡片出現在永遠不拿走 focus 的獨立 pane 中，讓我不會因為 LearnWhile 擋路而錯過權限提示。
4. 身為同時等待兩個 agent 的開發者，我希望卡片一直留著，直到*兩個* agent 都回來，這樣其中一個 agent 完成時，不會在我仍等待另一個 agent 時清掉卡片。
5. 身為開發者，我希望 agent 回來時做到一半的卡片，能在下一次等待時仍然存在，讓我可以完成跨兩次等待的 Review。
6. 身為開發者，我希望當我不在 Waiting 時，pane 顯示中性的 idle 狀態，讓 pane 內容永遠誠實告訴我是否應該正在複習。

**進行 Review**

7. 身為開發者，我希望先看到卡片的問題面，讓我真的嘗試回憶，而不是直接讀答案。
8. 身為開發者，我希望用單一按鍵揭示答案，讓短暫等待中的 reveal 不需要額外思考。
9. 身為開發者，我希望把自己的回憶評為 Again、Hard、Good 或 Easy，讓排程器得到正確間隔卡片所需的訊號。
10. 身為開發者，我希望按下評分的瞬間就持久化，讓 crash 或 terminal 關閉不會悄悄弄丟一次 Review。
11. 身為開發者，我希望即使我評自己 Again，Review 仍算完成，因為系統衡量的是我有沒有出現，而不是我答對了沒。
12. 身為開發者，我希望在同一次等待中評完一張卡後，下一張卡能立刻浮出，讓長等待可以容納多次 Review。
13. 身為開發者，我希望可以單純不回答卡片，讓我在等待時忽略 LearnWhile 不會有懲罰，也不會被 nag。

**保持誠實的排程**

14. 身為開發者，我希望優先看到真正到期的卡片，讓等待時間用在最重要的複習上。
15. 身為沒有到期卡的開發者，我希望能引入新的未看過卡片，讓等待仍然有用，而不是看到空 pane。
16. 身為開發者，我希望每天引入的新卡數有上限，避免 agent 使用量很高的一天一次把整副 deck 倒給我。
17. 身為開發者，我希望 LearnWhile 絕不在到期日前顯示卡片，讓我的 FSRS interval 保持準確，spacing benefit 不被削弱。
18. 身為 deck 很小的開發者，我希望有些等待顯示 idle 狀態是正常且不令人警覺的，讓我理解這是正確行為，而不是 bug。
19. 身為開發者，我希望我的排程資料是 FSRS-shaped，讓我投入這副 deck 的工作，在未來 import/export 到來時不會被困在這裡。

**執行 host**

20. 身為開發者，我希望在自己選定的 pane 中用單一 command 啟動 LearnWhile，讓我控制自己的 terminal layout，而不是讓工具重排它。
21. 身為開發者，我希望 LearnWhile 能跨許多 prompt 和等待持續執行，讓我每次工作只要啟動一次，然後忘了它。
22. 身為開發者，我希望用按鍵乾淨退出 LearnWhile，讓 terminal 留在正常狀態。
23. 身為 crash 後重啟 host 的開發者，我希望即使有 stale socket file，host 也能乾淨啟動，避免前一次不正常退出需要手動清理。
24. 身為開發者，我希望如果第二個 host 因為已有一個在執行而無法啟動，系統能清楚告知，讓我不會被兩個 pane 顯示不一致搞混。

**Fail-open**

25. 身為尚未啟動 LearnWhile 的開發者，我希望 coding agent 行為與過去完全相同，讓安裝 hook 在我不複習的日子沒有成本。
26. 身為 LearnWhile process crash 的開發者，我希望 agent 照常工作，因為 learning tool 絕不能拖垮我的真正工作。
27. 身為開發者，我希望 hook 不為提交 prompt 增加可感知 latency，讓我從不覺得安裝 LearnWhile 要付出成本。
28. 身為 host 還活著但卡住的開發者，我希望 hook 幾乎立刻放棄，讓 hung host 無法卡住 agent。
29. 身為開發者，我希望遺失的 Trigger close（來自 agent 或 host crash）最終會清掉，避免 phantom open 讓卡片永遠卡住。

**取得卡片**

30. 身為試用 LearnWhile 的開發者，我希望能從簡單檔案 seed 一副 deck，讓我不用透過尚不存在的 UI 手動輸入卡片，就能評估工具。
31. 身為開發者，我希望重新執行 seed 不會重複新增既有卡片，讓我能安全地迭代 seed file。
32. 身為開發者，我希望卡片與 review history 能在重啟後存活，讓 deck 隨著日子累積真正的排程狀態。

## 實作決策

**語言與形狀。** Rust。單一 binary 同時作為長駐 host 與 hook client（以 subcommand 選擇），因此 hook 繼承數毫秒啟動時間；鑑於它在每次 prompt submission 都會觸發，這是需求，不是偏好（ADR-0004）。Repo 目前沒有 `Cargo.toml` 或 `src/`；這份 spec 包含建立 crate。

**Process topology。** 依 ADR-0003，一個長駐 host 擁有 Runtime Engine、Learning Engine、Renderer 與 Storage。使用者自行把它放在 pane 中；沒有 multiplexer integration。Host 是 Unix socket 的唯一 binder，也是 SQLite 檔案的唯一 owner。

**Modules。** Host boundary 內有四個 internal module：

- *Runtime* - 擁有 open-Trigger set、接受 socket events、路由到 Learning 和 Renderer、擁有 Session lifecycle。
- *Learning* - 卡片、FSRS 排程、選擇順序、Review state machine。不知道 sockets 或 terminals。
- *Renderer* - 繪製目前卡片或 idle 狀態；沒有 business logic。
- *Storage* - SQLite；唯一發出 SQL 的 module。

Learning module 必須能以 injected clock 和 injected storage handle 建構。這是為了透過單一 seam 測試（見 Testing Decisions）所需，並且是 interface 的硬約束，不是建議。

**Socket protocol。** 透過 Unix domain socket 傳 newline-delimited JSON，路徑為 `$XDG_RUNTIME_DIR/learnwhile.sock`；若該變數未設定，fallback 到一個有文件說明的路徑。v1 中 frame 從 adapter 到 host 單向傳送；通道保留雙向能力，讓 post-v1 Prompt Gate 可以不用第二條通道就回覆（ADR-0004）。Frame 形狀：

```json
{ "v": 1, "type": "trigger_open", "adapter": "claude-code", "session": "<agent-session-id>", "at": "<rfc3339>" }
```

`type` 是 `trigger_open` 或 `trigger_close`。Trigger identity 是 `(adapter, session)` pair；ADR-0005 要求穩定 identity，讓 open 和 close 可以配對。Host 會忽略無法 parse 的 frame，並且絕不讓 bad frame 殺掉 accept loop。`v` 的存在是為了 protocol 演進；host 會安靜地拒絕未知版本。

**Open-Trigger set。** 以 `(adapter, session)` 為 key 的 set。`trigger_open` insert，`trigger_close` remove。Waiting 被定義為 set 非空。卡片在 empty->non-empty edge 浮出，在 non-empty->empty edge 清掉（ADR-0005）。同一 key 的 duplicate open 是 idempotent；unknown key 的 close 會被忽略。

**Lost-close recovery。** 解決 DESIGN_DRAFT §13 的 crash-recovery gap；見 ADR-0006。每個 open Trigger 都帶有從 open 起算的 expiry，預設 30 分鐘，並以 `trigger_expiry_seconds` 保存在 `config`。Sweep 由自己的週期 timer 執行，而不是由 frame arrival 驅動，並排掉 expired entries。Expiry 不會因後續 frame refresh，因為沒有 mid-turn traffic 可用於 refresh。

**Claude Code adapter。** Hook command。在 `UserPromptSubmit` 開啟 Trigger；在 `Stop`、`PermissionRequest` 或 `Elicitation` 中第一個出現者關閉 Trigger（ADR-0001）。它從 stdin 讀 Claude Code 的 hook JSON 取得 session id，以很短的 timeout 連線，寫出一個 frame，並無條件 exit 0，包括 connect failure、timeout、malformed input 或 panic。它不持有 learning state，也永遠不寫 storage。

**Card selection。** 依 ADR-0002 的嚴格順序，並在每次 surfacing 時重新評估：真正到期的卡片；否則如果今天 introductions 尚未達每日上限，就選新卡；否則 idle 狀態。絕不把尚未到期的卡片提前拉出來。「Today」依 injected clock 在使用者 local timezone 中解析。

**Review flow。** State machine，而不是 ad-hoc flags；它必須能在 agent 回來導致 mid-Review 被中斷時存活，並在同一 Session 之後的 Trigger 中恢復：

```text
Idle --surface--> Question --reveal--> Answer --rate(Again|Hard|Good|Easy)--> persist --> Idle
```

Review 只有在 persisted 後才算完成。明確不要求 correctness（CONTEXT.md）。In-flight card 是 Session state：當 set 變空而清掉 pane 時，不能丟掉它。Rating 來自 keypress；reveal 是單一按鍵。

**Schema。** SQLite，tables 依 DESIGN_DRAFT §9。這份 spec 解決 §13 的 card/FSRS data-model gap：

- `cards` - id、deck_id、front、back、FSRS state（`stability`、`difficulty`、`state`、`due`、`reps`、`lapses`、`last_reviewed_at`）、`created_at`，以及用於 seed idempotency 的 content hash。
- `review_history` - id、card_id、session_id、`reviewed_at`、`rating`，以及 FSRS stability/difficulty 的 before 和 after，外加 elapsed days 與 scheduled days。Append-only；它是 deferred Analytics Engine 之後會讀取的 audit trail，因此要記錄足夠資訊來重建 scheduler state。
- `decks` - id、name。v1 建立並使用單一 default deck；schema 中存在 decks，讓 post-v1 不需要 migration。
- `config` - key/value。保存 daily new-card cap 和 Trigger expiry。

Migrations 在 host startup 時執行。

**FSRS。** 使用既有 FSRS implementation，而不是自己手刻；`fsrs-rs` crate 是明顯候選，但在承諾前應確認它的 API surface。v1 採用 default parameters 即可，不做 optimization pass。

**Renderer。** `ratatui`。繪製目前卡片或 idle 狀態。這份 spec 解決 §13 的 idle-pane-content gap：idle 狀態顯示今日到期數、剩餘新卡數，以及下一次到期時間；足以分辨「沒有到期卡」與「不在等待」，也足以讓 pane 永遠不空白。

**Card seeding。** 因為手動輸入不在範圍內，`seed` subcommand 會把 tab-separated front/back file 匯入 default deck，並跳過 content hash 已存在的 rows。這是讓 v1 可用的 developer affordance，明確**不是**延後的 Anki-compatible Import/Export feature，也不應累積格式支援。

## 測試決策

**這裡好的測試長什麼樣子。** 測試只 assert 開發者能觀察到的東西：pane 顯示什麼，以及 database 中最後有什麼。沒有測試會直接伸進 open-Trigger set、Review state machine 或 selection function。若某個測試會因 internals 重構而壞掉，但開發者體驗沒有改變，那就是壞測試，應該重寫。

**一個 seam：host boundary。** 已與開發者確認。測試會以 in-process 方式啟動 host，並注入三個 dependencies：temp SQLite path、temp socket path，以及 controllable clock；接著透過撥打**真正的 Unix socket**並寫入**hook 會寫的同一種 frame**來驅動它。這測到實際 protocol 與真正的 open-Trigger set，而不是兩者的 mock。Learning Engine 刻意*不*給自己的 seam，因此沒有測試能在 wired-up path 壞掉時仍然通過。

Injected clock 不是 optional：FSRS 依賴時間，而用 real clock 測 due-date behavior 是 non-deterministic。測試 due dates 與 daily cap rollover 時，時間會被明確推進。

**同一 seam 的兩個 input channels。** Socket 承載 Triggers；keypresses 承載 reveal 和 rating。兩者都是同一 host boundary 的 inputs，因此 event loop 需要一種測試可達的方式來注入 key events。這是 loop 的設計約束，應該早點決定，而不是之後補上。

**兩個 observation surfaces。** `ratatui` 的 `TestBackend` 會 render 到 in-memory cell buffer，測試對它 assert 開發者看到什麼。Direct SQLite reads 確認持久化了什麼。兩者對被測 modules 都是外部觀察。

**測什麼。** Trigger open 會浮出卡片，close 會清掉它；兩個重疊 Trigger 會讓卡片保留到兩者都 close；lost close 會過期並排掉；in-flight card 能在 agent 回來後存活，並在下一個 Trigger 恢復；完整 reveal-and-rate flow 會寫入一列 `review_history` 並推進卡片 due date；selection 嚴格遵循 due -> new -> idle；not-yet-due card 絕不被浮出；daily cap 會維持並在隔天 rollover；idle 狀態顯示正確 counts；seeding 是 idempotent。

**Fail-open 在 adapter 另行測試。** Hook 會以 subprocess 呼叫，情境包括沒有 host、refused socket，以及刻意 wedged socket；每種情況下，它都必須在 timeout 內 exit 0。這是唯一值得使用 subprocess test 的地方，因為「真正的 binary exits 0」正是主張本身，而且無法對 in-process harness 作出此主張。

**Prior art。** 無；這是 repo 的第一批 code，因此這些測試會設定 pattern。這代表早點把 harness 做好很重要：一個設計良好的 `spawn_test_host` helper 是這份 spec 中 leverage 最高的東西。

**不測什麼。** FSRS interval math 本身；那屬於 upstream crate 的 test suite，在這裡重測會讓我們耦合到它的 internals。我們測的是有呼叫它，並持久化它回傳的內容。

## 範圍外

- **手動新增卡片**（CLI 或 in-TUI）。卡片來自 `seed`。DESIGN_DRAFT §13 的 card-add UX gap 保持開放。
- **Contract Engine** - Learning Contracts 與 Prompt Gates。v1 絕不阻塞任何東西。Socket 保留雙向能力，因此之後不需要 rework。
- **Analytics Engine。** `review_history` 記錄得足夠豐富，之後可以餵給它，但 v1 沒有東西讀它。
- **Import / Export**（Anki TSV、CSV、JSON）。`seed` subcommand 不是這件事。
- **額外 Trigger Adapters**（Codex、OpenCode）。Protocol 以 adapter key 區分，因此它們可以不碰 Runtime 就接入。
- **自動開啟 tmux/Zellij panes 與 Desktop Renderer。** 使用者自行安排 layout（ADR-0003）。
- **UI 中的 multiple decks。** Schema 支援它們；v1 使用一個 default deck。
- **FSRS parameter optimization。** 只用 defaults。
- **Windows。** 舊版 Windows 不原生支援 Unix sockets（ADR-0004）；v1 只支援 Unix。
- **Multi-host / multi-user。** 一個 host 擁有 socket 與 database。

## 補充說明

**三個決策已提升為 ADR。** Trigger expiry policy（ADR-0006）、newline-delimited JSON frame format（ADR-0007），以及 single-binary-with-subcommands topology（ADR-0008）原本來自這份 spec，現在位於 `docs/adr/`，成為 durable record。若這份 spec 與那些 ADR 不一致，以 ADR 為準。

**這裡沒有任何內容與既有 ADR 矛盾。** 這份 spec 承諾了 ADR-0004 延後的「concrete IPC transport and message format」，也解決了 ADR-0005 明確標出的 lost-close tolerance；兩者都是那些 ADR 刻意留下的 gap，而不是推翻。

**順序。** Spine 應該先於 Learning Engine 落地：Trigger -> socket -> set -> Renderer with a hardcoded card，可以先證明風險最高、最難回頭的部分（hook 是否在我們以為的時候觸發、fail-open 是否成立），趁變更還便宜時調整。FSRS 與 storage 是較常規的工作，可以之後更有把握地接上。

**正在測試的假設。** DESIGN_DRAFT §3 把 v1 描述為測試*開發者會不會在 AI waits 中複習？*值得注意的是，這個 build 無法完全回答它：`review_history` 保存資料，但在 Analytics Engine 存在前沒有東西呈現它。第一輪 dogfood 時手動讀它沒問題；只是不要把 shipping this 誤認為已經有答案。
