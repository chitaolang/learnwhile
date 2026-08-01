# Prompt Gate 讓 UserPromptSubmit 這一次交換變成 request/response

**背景。** Trigger frame 是以 newline 分隔的 JSON，單向送出且 fire-and-forget（ADR-0007）：hook 寫一個 frame 然後結束，host 解析並套用它，什麼都不回傳。Prompt Gate（ADR-0014）需要在 prompt 能繼續之前拿回一個裁決，所以至少一次交換必須變成 request/response。問題是要不要在其他所有地方保留單向模型。

**決策。** 只有 `UserPromptSubmit` 這次交換，而且只在 hook 帶著 `--gate` 執行時，才變成 request/response：hook 送出標記為 gate 查詢的 open 意圖，並在它既有的有界逾時內等待 allow 或 block 裁決，然後要嘛繼續並讓 open 登錄，要嘛阻擋且不送出 open。其他每個 frame，以及 gate 關閉時的 `UserPromptSubmit`，都維持單向 fire-and-forget。回覆以 hook 的寫入與讀取逾時為界，沒有回覆就 fail open。被拒絕的方案：讓所有 frame 都變成 request/response，那會為了一個只有 gate 需要的回覆，違反 ADR-0008 地把每個 hook event 都變熱。

**後果。** host 的 listener 今天是解析後丟棄 frame，現在為這一次交換多了一條回覆路徑，在單一 event loop 上回答（ADR-0009），所以不引入新的並行。協定不再是純單向的，但這個例外很窄且需選擇啟用，而 fire-and-forget 模型仍然統管其他一切。這是擴充 ADR-0007，而不是取代它。規格：[`docs/specs/prompt-gate.md`](../../specs/zh-TW/prompt-gate.md)。
