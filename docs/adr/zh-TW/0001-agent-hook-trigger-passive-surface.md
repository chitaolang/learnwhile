# 觸發由 AI agent 生命週期 hook 驅動，並以被動方式呈現

**背景。** LearnWhile 的前提是把等待 AI 的時間轉成複習時間。我們考慮過 terminal idle detection 和手動呼叫，但選擇由 coding agent 自己的生命週期事件來驅動 Trigger（透過每個 agent 專屬的 Trigger Adapter，例如 Claude Code hooks），因為這是唯一真正能分辨「正在等 AI」與其他閒置時間的來源。

**決策。** Agent Trigger 發生時浮出一張卡片，agent 回來時清掉卡片。預設呈現方式是**非阻塞側邊 pane**（tmux/zellij），永遠不搶 foreground focus，因此開發者不會錯過權限提示或 agent 完成工作的時刻。只有在明確選擇加入**學習契約**（Learning Contract，例如 Prompt Gate）時才允許阻塞；沒有契約時，系統是 fail-open，任何東西都不會被阻擋。

**後果。** 整合會與 agent 綁定：每個支援的 agent 都需要自己的 Trigger Adapter。被動預設代表除非使用者選擇加入 Contract，否則習慣不會被強制執行。每個 adapter 的 hook 細節（哪些事件映射成 Trigger、如何偵測「agent 回來了」）會延後到各 adapter 的設計中決定。
