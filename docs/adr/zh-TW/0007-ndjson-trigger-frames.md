# Adapter 傳送以 adapter 與 session 識別的 newline-delimited JSON frame

**背景。** ADR-0004 決定了 transport（Unix domain socket），並明確保留 message format 尚未決定。格式必須同時滿足幾件事：短生命週期 hook process 要能輕易發出、帶有穩定 Trigger identity 讓 open 和 close 可以配對（ADR-0005）、能在垃圾或部分輸入下存活而不拖垮 host，並保留空間，讓未來 opt-in Prompt Gate 需要同通道回覆時 host 可以回應。流量微不足道，每個 agent 回合只有幾個 frame，因此 wire efficiency 幾乎無關緊要；可除錯性與非 Rust adapter 的撰寫容易度則很重要。

**決策。** 每行一個 UTF-8 JSON object，以 newline 結尾：

```json
{ "v": 1, "type": "trigger_open", "adapter": "claude-code", "session": "<agent-session-id>", "at": "<rfc3339>" }
```

`type` 是 `trigger_open` 或 `trigger_close`。Trigger identity 是 `(adapter, session)` pair。Host 以逐行方式讀取，並設 bounded maximum line length；對於不認得的 `v` 會忽略，任何無法 parse 的 line 也會忽略。任何情況下，bad frame 都不能殺掉 accept loop。被拒絕的選項包括：length-prefixed binary 或 `bincode`，速度較快但 wire 上不可讀，且對其他語言撰寫的 adapter 不友善；protobuf 或 msgpack，其 schema tooling 對兩種 message type 來說過度；以及 bare positional text，因為它沒有空間在不破壞所有 adapter 的情況下增加欄位。

**後果。** 相對於承載的資訊，格式很 verbose，但在這個流量下無關緊要。Maximum line length 是必要條件，而不是防禦性潤飾：沒有它時，有 bug 或惡意的 client 可以串流一條永不結束的 line，耗盡 host memory。安靜地忽略 malformed input 維持了 fail-open 姿態，但也讓 adapter bug 隱形；因此 host 需要 log file，讓 discarded frames 可以被診斷。Pane 不能承擔這個目的，因為它必須維持被動（ADR-0001）。因為 framing 是對稱的，未來 Prompt Gate reply 可以用新的 `type` 重用同一格式，不需要 transport work。`v` field 只多花一個 key，卻讓未來 adapter 可以依照 frozen contract 建置。
