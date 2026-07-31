# Open Trigger 會過期，因此遺失 close 不會讓卡片永遠卡住

**背景。** ADR-0005 定義開發者「Waiting」的精確條件是 open-Trigger set 非空，並指出 host 必須容忍遺失 close，但尚未解決做法。Close 可能因多種方式消失：agent 在回合中 crash、host 在 Trigger open 時重啟、hook 沒有在 `Stop` 觸發，或 adapter 有 bug。這些都會洩漏 phantom open；而因為單一洩漏 entry 就會讓集合保持非空，卡片會永久留在畫面上，卡片可見性也不再帶有任何資訊。光靠 session-end sweep 無法修復這件事：ADR-0004 讓 adapter fire-and-forget，沒有持久連線，因此「adapter disconnected」不是可觀察事件。Adapter heartbeat 同樣不可用，因為 hooks 只會在 agent 發出事件時執行，而長回合期間缺少的正是這種事件。

**決策。** Open-Trigger set 中的每個 entry 都帶有 expiry，從 Trigger open 的時間開始計算，預設 30 分鐘，並以 `trigger_expiry_seconds` 存在 `config` 中，而不是 hardcoded。Sweep 由週期 timer 執行，獨立於 frame arrival，並移除過期 entry；如果這讓集合變空，卡片就會像真正收到 close 一樣清掉。Expiry 刻意*不*因後續 frame 而 refresh：在只有 open 和 close frame 被定義的情況下（ADR-0007），回合中沒有任何 traffic 可以用來 refresh，因此 refresh 規則只是沒有作用的複雜度。若 adapter 未來發出中途進度 frame，refresh 才會有意義，到時可以重新檢視。

**後果。** 兩種失敗方向是不對稱的，這也正是偏短到期時間合理的原因：太早過期會在開發者仍等待時清掉卡片，稍微惱人，但下一個 Trigger 會自行修正；太晚或永不過期則會把 stale card 無限期釘在畫面上，並無聲地摧毀這個 surface 的意義。真正不間斷且超過 30 分鐘的 agent 回合會在等待中途失去卡片；這被接受，而 config key 是為那些 agent 經常跑更久的開發者準備的。Sweep 必須由自己的 timer 驅動：如果掛在 frame arrival 上，沒有後續 traffic 的 phantom open，也就是本 ADR 正在處理的情境，就永遠不會排掉。因為 expiry 是 time-based 而不是 liveness-based，host 完全不需要追蹤 adapter process。
