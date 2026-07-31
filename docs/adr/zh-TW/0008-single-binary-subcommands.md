# 同一個 binary 同時作為長駐 host 與 hook client

**背景。** ADR-0003 把系統拆成長駐 host 和輕量、agent-specific adapter，但保留 adapter 實際上如何發佈與呼叫尚未決定。決定這件事的約束是 latency：Claude Code adapter 會在每次 `UserPromptSubmit` 執行，直接位在開發者提交 prompt 的路徑上，而 ADR-0004 要求它近乎立即返回，這樣 LearnWhile 才不會被感覺成 agent 的成本。無論 adapter 是什麼，它的 cold-start cost 都會由開發者每小時支付很多次。

**決策。** 發佈單一 binary，並以 subcommand 選擇角色：預設 invocation 執行 host（socket、Runtime、Learning、Renderer、Storage）；`hook` 作為 Trigger Adapter，從 stdin 讀取 Claude Code 的 hook JSON 並寫出單一 frame；`seed` 匯入卡片。Rust binary 可以在個位數毫秒內啟動，舒服地落在預算內。被拒絕的選項包括：獨立 adapter binary，原則上更乾淨，但會產生兩個可能彼此版本漂移的 artifact，也讓使用者安裝的東西加倍；以及用 shell script 驅動 `nc` 或 `socat`，雖然不需要 build，但依賴的工具不一定存在，對 Unix socket 的支援也各異，且很難精準控制 send timeout 和 exit code，而這兩件事正是 adapter 絕對必須做對的。

**後果。** `hook` path 必須保持 cold：不能開 SQLite、不能初始化 TUI、除了解析 socket path 之外不能載入 config。這是一項持續紀律，而不是一次性變更，因為 shared startup code 是初始化自然會累積的地方，會悄悄把本 ADR 想避免的 latency 帶回來。Hook path 的工作量保持最小，值得用測試量測並 assert，而不是只憑意圖。Binary size 會因為 hook 帶著 host code 而變大，但對本機安裝的 developer tool 來說無關緊要。單一 artifact 讓同一台機器上的 host/adapter version skew 不可能發生，所以 ADR-0007 的 `v` field 只對未來第三方或非 Rust adapter 有價值；但這仍然足以成為保留它的理由。
