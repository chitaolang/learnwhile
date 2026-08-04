# Runtime 追蹤 open Trigger 集合；「等待中」代表集合非空

**背景。** 多個 agent 共享同一個 host，因此不同 adapter 的 Trigger 會重疊。如果 Runtime 只追蹤單一 active Trigger，其中一個 agent 回來時就會清掉卡片，即使開發者仍在等待另一個 agent。卡片可見性必須反映聚合後的閒置狀態，而不是任何單一 agent 的狀態。

**決策。** Runtime 維護目前 open 的 Trigger 集合，以 adapter 和 agent session 作為 key。開啟 Trigger 會新增一個 entry；關閉 Trigger（Stop / Notification）會移除該 entry。只要集合非空，開發者就正好處於「等待中」：集合非空時浮出卡片，集合變空時清掉卡片。

**後果。** Runtime state 是 set/refcount，不是單一 slot。Adapter 必須送出穩定的 Trigger identity，讓 open 和 close 可以配對。Host 必須容忍遺失的 close（agent 或 host 在 Trigger 期間 crash），避免集合洩漏 phantom open，讓卡片永遠留在畫面上；可透過每個 Trigger 的 timeout 和/或 session-end sweep 達成。Rolling Session 會跨越這個集合多次非空/空的 cycle。
