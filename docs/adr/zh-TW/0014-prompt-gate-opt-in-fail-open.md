# Prompt Gate 以 hook flag 選擇啟用，且 fail-open

**背景。** LearnWhile 是被動的（ADR-0001）也是 fail-open 的（ADR-0004）：從不阻擋開發者或 agent。有些開發者想為自己套上相反的東西，一個承諾裝置：在他們完成一次 Review 之前，先壓住下一個 prompt。領域模型已經把這件事命名為 **Prompt Gate**，一種 **Learning Contract**。要做出它，不能為任何沒有選擇啟用的人破壞 fail-open，也不能讓 hook 路徑變熱，因為 hook 不載入 config 且在每個 prompt 上執行（ADR-0008）。hook 無法便宜地讀取儲存的設定，所以它無法在不讓每個使用者的每次送出都付出成本的情況下，從 `lw config` 得知 gate 是否開啟。

**決策。** Gate 以每個 hook 註冊為單位選擇啟用，做法是把 `UserPromptSubmit` 的 hook 指令改成 `learnwhile hook --gate`。沒有這個 flag 時，hook 就是 v1 的冷路徑：fire-and-forget 送出一個 `TriggerOpen`、沒有往返、沒有裁決。有這個 flag 時，且只在 `UserPromptSubmit` 上，hook 對 host 做一次有界、fail-open 的往返，取得 allow／block 裁決。當 host 連不上、回覆逾時、或沒有東西可複習時，gate 絕不阻擋。被拒絕的方案：在 hook 路徑上讀取 `lw config` 的 key，那會讓 hook 變熱（違反 ADR-0008），或強迫一個永遠開著的往返，讓從未選擇啟用的開發者也被課稅。

**後果。** 選擇啟用住在 `settings.json`，而不是 `lw config`，所以它比較不容易被發現，而切換它意味著編輯 hook 指令。不論 flag 如何，host 都追蹤 review debt，所以啟用 gate 不需要重啟 host。Gate 可以藉由移除 flag 或關掉 host 被輕易繞過，這是對的：它是一個自我施加的承諾裝置，不是鎖，而 fail-open（ADR-0004）永遠不為它犧牲。規格：[`docs/specs/prompt-gate.md`](../../specs/zh-TW/prompt-gate.md)。
