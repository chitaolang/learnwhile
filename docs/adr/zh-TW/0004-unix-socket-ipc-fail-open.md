# Adapter 透過 Unix socket 連到 host；架構上 fail-open

**背景。** ADR-0003 留下 IPC transport 尚未決定。「Fail-open」是核心原則：停掉或變慢的 host 絕不能卡住使用者的 agent，而會阻塞或出錯的 hook 正會造成這件事。因此 transport 的選擇*就是* fail-open 機制。我們也希望未來能便宜地加入 opt-in Prompt Gates（v1 延後），這需要一條 host 可以回覆的通道。

**決策。** Trigger Adapter 透過已知路徑的 Unix domain socket（例如 `$XDG_RUNTIME_DIR/learnwhile.sock`）連到長駐 host。Hook 以 fire-and-forget 方式送出 Trigger event，使用很短的 connect/send timeout，並吞掉所有錯誤（永遠 exit 0）。Socket 不存在或被拒絕時會立刻返回，因此 host 掛掉時就是安靜的 no-op。Host 是 socket 的唯一 binder，這也符合它對 SQLite 的唯一 ownership。因為 socket 是雙向的，同一條通道之後也能把 Review result 回傳給 agent，用於 opt-in Prompt Gate；不需要第二條通道。

**後果。** Host 需要在 TUI 旁邊執行 socket accept loop，並在啟動時 unlink stale socket。仍然需要 send timeout，才能在 host 還活著但卡住時維持 fail-open（只有 connect failure 會立刻返回）。舊版 Windows 不原生支援 Unix sockets，所以未來 Windows renderer 需要替代 transport。這個方案勝過 watched state file（比較簡單但單向，gating 需要第二條反向通道）以及 localhost HTTP（活動零件更多：port、server、timeout）。
