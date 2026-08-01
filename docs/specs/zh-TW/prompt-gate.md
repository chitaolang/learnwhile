# Prompt Gate：一次 Review 可以擋住下一個 prompt

詞彙沿用 [`/CONTEXT.md`](../../../CONTEXT.md)。決策以 ADR-NNNN 引用自 [`docs/adr/`](../../adr/)。本規格屬 v1 之後，且需選擇啟用：gate 關閉時，系統的行為與 v1 完全相同，全然 fail-open。

## 問題陳述

LearnWhile 把等待時間轉成複習，但它從不強迫。一個想養成習慣的開發者仍然可以跳過每一張卡片，因為這個表面在設計上就是被動的（ADR-0001），而且什麼都不會被阻擋（ADR-0004）。對某些開發者來說，那份被動正是整個問題所在：他們真心想做的複習，每一次都輸給下一個 prompt，而那正是這個產品存在要解決的失敗。

他們想要一個承諾裝置：在我完成一次複習之前，先壓住我送給 agent 的下一個 prompt。領域模型已經把這件事命名。一個 **Learning Contract** 是一個選擇啟用的承諾，讓一次 Review 的結果去 gate 一個動作，而一個 **Prompt Gate** 就是那個要求「在 agent 的下一個 prompt 繼續之前，先完成一次 Review」的特定 Contract。本規格建造這一個 Contract。

張力在於：這是對兩個基礎決策刻意開的例外。它不能為任何沒有選擇啟用的人破壞 fail-open（ADR-0004），不能讓 hook 路徑變熱（ADR-0008），也不能讓開發者陷入死結，或讓 LearnWhile 以 ADR-0001 禁止的方式「擋在 agent 面前」。

## 解法

在 `UserPromptSubmit` 的 hook 指令加上 `--gate` 來選擇啟用。沒有這個 flag 時，hook 就是今天那個冷的、fire-and-forget 的 adapter。有它時，每個 prompt 上 hook 會對 host 做一次有界、fail-open 的往返，問「我欠著一次複習嗎？」如果欠著，hook 擋住這個 prompt，而 Trigger 不會 open。如果沒欠，prompt 繼續，Trigger 照常 open。

**review debt（複習債務）**是 gate 讀取的狀態。在有卡片在螢幕上時交棒，會產生一筆債務。完成任何一次 Review 就清除它。如果沒有東西可複習，就不產生債務，所以 gate 絕不會把開發者困在「沒有東西可還」的處境。因為阻擋落在開發者不在 Waiting 時，而卡片通常只在 Waiting 時顯示，所以啟用中的 gate 也會**在閒置時顯示欠著的卡片**，讓債務隨時可以當場償還。給它評分就清除債務，下一個 prompt 就過去了。

每一條不確定的路徑都 fail open：gate 關閉、host 連不上、回覆逾時、或沒有東西可複習，都讓 prompt 繼續。Gate 是一個自我施加的承諾裝置，不是鎖。移除 flag 或關掉 host 就繞過它，這是對的。

## 使用者故事

1. 身為一個在養成複習習慣的開發者，我想在我完成一張卡片之前，我的下一個 prompt 被壓住，讓我想做的複習不再輸給下一個 prompt。
2. 身為同一個開發者，我想在我看到債務的當下就從 pane 清掉它，即使我正在兩個 prompt 之間閒置，這樣阻擋永遠不會讓我無事可做。
3. 身為一個沒有選擇啟用的開發者，我想在 hook 路徑上零改變、零額外延遲，這樣 gate 對沒要求它的人不花任何成本。
4. 身為一個 LearnWhile 沒在跑的開發者，我想我的 prompt 原封不動地通過，這樣一個背景工具永遠不能卡住我的 agent。
5. 身為開發者，我想把 gate 關掉就立刻回到被動、從不阻擋的行為，這樣選擇啟用永遠不是一扇單向門。

## 選擇啟用與冷 hook

Gate 以每個 hook 註冊為單位啟用，而不是在 `lw config`。`UserPromptSubmit` 的 hook 指令變成 `learnwhile hook --gate`。這是唯一能讓 gate 關閉路徑維持像 v1 一樣冷（ADR-0008）的方法：hook 不載入 config，也無法便宜地得知一個儲存的設定，所以選擇啟用必須寫在呼叫本身。

- **`learnwhile hook`**（無 flag）：不變。在 `UserPromptSubmit` 上它 fire-and-forget 送出一個 `TriggerOpen` frame 然後以 0 結束。沒有裁決、沒有往返。
- **`learnwhile hook --gate`**：在 `UserPromptSubmit` 上它執行下一節的 request／response。在其他每個 event 上，它的行為與無 flag 的 hook 完全相同（這個 flag 只改變送出這一次交換）。

不論 flag 如何，host 都追蹤 debt，所以啟用 gate 不需要重啟 host。host 在這個 Session 第一次收到 gate 查詢時得知它正在使用 gate，也只有到那時才會在閒置時顯示欠著的卡片（見下文），所以從不傳 `--gate` 的開發者永遠看不到 idle pane 有任何改變。

## Gate 交換

在帶著 `--gate` 的 `UserPromptSubmit` 上，hook 與 host 做一次 request／response（ADR-0016 為的正是這一次交換而擴充了 ADR-0007 的單向 frame）：

1. hook 送出 open 意圖，標記為 gate 查詢，並在它既有的有界逾時內等待一個裁決。
2. host 回覆 **allow** 或 **block**：
   - **allow**：沒有欠著債務。host 登錄這次 Trigger open（交棒真的發生了），hook 以 0 結束、沒有任何阻擋輸出。prompt 繼續。
   - **block**：欠著一筆債務。host **不**登錄 Trigger open（交棒沒有發生），hook 印出 `{"decision":"block","reason":"Finish one review to continue."}` 並以 0 結束。Claude Code 擋住這個 prompt 並把 reason 顯示給開發者。
3. 如果 host 沒有及時回覆、拒絕連線、或沒在跑，hook 就 fail open：它就像無 flag 的 hook 一樣繼續（fire-and-forget open、以 0 結束、不阻擋）。

用 `{"decision":"block"}` 而不是 exit 2 是刻意的：`reason` 顯示給開發者，而不是當成錯誤餵給 Claude。gate 說話的對象是開發者，不是 agent。

## Review debt

Debt 是 host 裡一個以 Session 為範圍的布林值，在記憶體中，不持久化。它承接 lapse queue 的先例（ADR-0010）：以 Session 為範圍、活不過 host process（ADR-0011）的記憶體內複習狀態。重啟會清掉它，這是可接受的，且偏向 fail-open。

- **產生**：在一次等待中有卡片被浮到 pane 時。選擇產生 idle 狀態（沒有到期卡、也沒有在上限內的新卡）不產生任何債務。
- **清除**：靠任何一次完成的 Review，也就是任何一個評分。不要求正確，Again 一樣清除它，與 **Review** 和 **Lapse** 的定義一致。
- **一張 in-flight 卡片**（已揭示但未評分）仍算欠著。揭示不等於完成。
- **讀取**：由 gate 交換讀取，在查詢的當下若欠著債務就 block。
- **多 agent**：一個 Session 範圍的旗標。任何被浮出的卡片都會武裝它，而一次 Review 償還它，這符合「每段閒置至少一次複習」，而不是「每個 agent 一次」。

## 在閒置時償還債務

一個硬性的 gate 在 `UserPromptSubmit` 阻擋，而那是交棒，所以阻擋總是落在開發者**不在 Waiting** 時。今天卡片只在 Waiting 時繪製（`host.rs`，那些 `ReviewView::Question { .. } if waiting` 分支），所以被阻擋的開發者螢幕上沒有卡片可複習，也沒辦法叫出一張，因為叫出一張需要一個 prompt，而 prompt 被擋住了。這是一個死結，唯一的逃生口是關掉 LearnWhile。

所以啟用中的 gate 會在閒置時顯示欠著的卡片：當 host 這個 Session 已經看過一次 gate 查詢、且欠著債務、且開發者不在 Waiting 時，pane 繪製那張欠著的卡片（目前選中的、in-flight 的卡片），而不是 idle 狀態。給它評分就清除債務，pane 回到 idle。這是 Learning Contract 概念明確允許的「閒置即被動」例外（ADR-0015）。它從不取得前景焦點、也從不藏起 agent 的表面，所以 ADR-0001 的字面承諾守住。

## 範圍：沒有例外

Gate 在欠著債務時的每一個 `UserPromptSubmit` 上開火，包括一個回答 agent 權限或輸入請求的 prompt。LearnWhile 從不藏起那個請求、也從不偷走焦點，所以你不會錯過它（ADR-0001），但你可能得先完成一次複習才能回答它。這是嚴格範圍被接受的代價，而 pay-while-idle 讓它不會死結。豁免回覆曾被考慮並拒絕（ADR-0015）：它不會移除死結（一個乾淨 `Stop` 之後的全新 prompt 也會在閒置時被擋，所以無論如何都需要 pay-while-idle），而且會削弱這個承諾。如果 dogfooding 顯示這咬得太痛，豁免是第一個該重新考慮的東西。

## Fail-open 對照表

| 情況 | 結果 |
|---|---|
| hook 沒有 `--gate` | fire-and-forget open、以 0 結束。沒有往返。與 v1 相同。 |
| `--gate`，host 沒在跑或拒絕 | fail open：繼續、open、以 0 結束。 |
| `--gate`，回覆超過逾時 | fail open：繼續、open、以 0 結束。 |
| `--gate`，沒欠著債務 | allow：繼續、open、以 0 結束。 |
| `--gate`，欠著債務 | block：不 open、印出 block reason、以 0 結束。 |

每一列都以 0 結束。gate 只可能在最後一列加上一個 block。

## 什麼不變

- **排程與 storage。** FSRS、選卡（ADR-0002）、schema 與 `review_history` 都不動。Debt 是記憶體內的 Session 狀態，沒有 migration。
- **Review flow。** space 揭示、1 到 4 評分、每個評分一列 history。gate 讀取結果，它不改變一次 Review 如何運作。
- **非送出的 hook event。** `Stop` 與 `Notification` 仍然關閉 Trigger；`PreToolUse` 與其他仍然被忽略（`hook.rs`）。只有 `UserPromptSubmit` 這次交換改變，而且只在 `--gate` 之下。
- **預設姿態。** 在任何地方都沒有 `--gate` 時，什麼都不阻擋、hook 維持冷、idle pane 從不顯示卡片。v1 行為逐位元組保留。

## 邊界情況

- **一個 Session 的第一個 prompt。** 沒有先前的交棒、沒有債務，所以它通過。
- **Session 中途 deck 用盡。** 沒有卡片被浮出，所以不產生新債務，gate 不阻擋。
- **gate 開著時 host 重啟。** Debt 重設為 false。重啟後第一個 prompt 通過；下一次等待正常重新武裝。
- **在閒置時複習。** 在啟用中的 gate 下欠著債務時，揭示與評分鍵作用在 idle pane 顯示的那張欠著的卡片上，就跟在 Waiting 時一樣。
- **設了 `--gate` 但 host down。** fail open。gate 永遠只跟 host 一樣存在。

## 測試

遵循 repo 邊界優先的規則（斷言 pane、database、以及 hook 可觀察的輸出，絕不斷言內部）：

- **gate 關閉是隱形的。** 沒有 `--gate` 時，一個完整的 prompt-wait-prompt 週期從不阻擋，idle pane 從不顯示卡片，與 v1 相符。hook 不做往返。
- **欠著時 block。** gate 開著、一張卡片被浮出且未評分，然後一個 `UserPromptSubmit`：hook 送出 block 裁決，而 host 不 open Trigger。
- **還清後 allow。** gate 開著、被浮出的卡片被評分，然後一個 `UserPromptSubmit`：allow，Trigger open。
- **沒東西可複習時 allow。** gate 開著、一次 idle 等待（空的或用盡的 deck），然後一個 prompt：allow。
- **在閒置時償還。** gate 開著，在一次有未還卡片的等待之後，pane 在不在 Waiting 時顯示欠著的卡片；給它評分清除債務，下一個 prompt 被 allow。
- **fail-open。** gate 開著但 host 停掉：hook 在它的延遲預算內以 0 結束、沒有 block 輸出。
- **延遲。** 帶 gate 的 `UserPromptSubmit` 往返維持在一個有界的預算內，以 ADR-0008 的 hook 延遲測試那樣在真實 binary 上斷言。

## 要升格為 ADR 的決策

起草為 ADR-0014 到 ADR-0016（先英文，zh-TW 隨承接 0001 到 0013 的同一輪補上）：

1. **Prompt Gate 以 hook flag 選擇啟用且 fail-open**（ADR-0014）。由 `--gate` 啟用、預設關閉、關閉時冷、在每個不確定處 fail-open。被拒絕：在 hook 路徑上讀取 `lw config` 的 key（會讓 hook 變熱，違反 ADR-0008）。
2. **啟用中的 gate 作用在送出的 prompt 上，並在閒置時顯示欠著的卡片**（ADR-0015）。解掉死結，並記錄「閒置即被動」的例外與「沒有例外」的範圍。
3. **gate 讓 `UserPromptSubmit` 這次交換變成 request／response**（ADR-0016）。為這一次交換擴充、而非取代 ADR-0007 的單向 frame。

本規格也收緊了 **Fail-open** 與 **Learning Contract** 這兩個 glossary 條目，讓「unmet」不再讀成「即使選擇啟用也絕不阻擋」。一個選擇啟用的 Contract 正是在它的 Review 要求真的未被滿足時才阻擋；fail-open 統管無法評估與未選擇啟用的情況。

## 範圍之外

- **其他 Learning Contract**（gate 某個特定工具、每個 deck 的每日目標、一個時間盒）。Prompt Gate 是第一個也最簡單的 Contract；機制不該過度貼合它，但這裡也不建造其他的。
- **嚴格度模式**（nag-once、typed bypass）。在設計時被考慮並延後，偏好硬性阻擋。若硬性阻擋咬得太痛，這些延後的選項仍是自然的洩壓閥。
- **跨重啟持久化 debt。** 依決策是以 Session 為範圍且在記憶體中，就像 lapse queue。
- **gate 摩擦的分析。** 屬於 Analytics Engine，而那完全在 v1 範圍之外。
