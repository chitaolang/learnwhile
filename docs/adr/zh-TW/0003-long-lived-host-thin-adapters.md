# LearnWhile 以長駐程序執行；adapter 是輕量 IPC client

**背景。** 被動側邊 pane 呈現（ADR-0001）需要一個持續存在的 viewport，並保存 rolling Session 的狀態；但 Claude Code hooks 是短生命週期的 command invocation，既不能擁有 pane，也不能 host 一個 TUI。系統中必須有某個東西活得夠久，才能顯示卡片並保存複習狀態。

**決策。** LearnWhile 以單一長駐程序執行，host Runtime Engine、Learning Engine、Storage 與 Renderer，並位在使用者自行安排的 pane 或 terminal 中（tmux/zellij split 或第二個 terminal 是使用者環境，不是 v1 功能）。Trigger Adapter 是輕量 client；以 Claude Code 來說，它是一個 hook command，透過 IPC 把 Trigger open/close 事件轉送給正在執行的程序。長駐程序是 SQLite 檔案與複習迴圈的唯一 owner。

**後果。** 這調和了「Terminal renderer / defer tmux+zellij」：Renderer 會畫在它被給定的任何 pane 中；自動開啟 multiplexer pane 是延後的整合。v1 需要使用者啟動程序，並把它放在 agent 旁邊；如果它沒有執行，adapter events 必須 no-op（fail-open，見 IPC/fail-open 決策）。同一時間只能有一個程序擁有 SQLite 檔案；多個並行 agent 共享同一個 host。具體的 IPC transport 與 message format 另行決定。
