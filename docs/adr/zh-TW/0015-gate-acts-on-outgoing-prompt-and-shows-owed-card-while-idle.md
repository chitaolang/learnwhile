# 啟用中的 Prompt Gate 作用在送出的 prompt 上，並在閒置時顯示欠著的卡片

**背景。** 一個硬性的 Prompt Gate（ADR-0014）在 `UserPromptSubmit` 阻擋，而那正是交棒的時刻，所以阻擋總是落在開發者「不在 Waiting」時。但卡片只在 Waiting 時才繪製（`host.rs`，那些 `ReviewView::Question { .. } if waiting` 的分支）。因此被阻擋的開發者螢幕上沒有任何東西可複習，也沒有辦法叫出一張卡片，因為叫出卡片需要一個 prompt，而 prompt 被擋住了。這是一個死結，唯一的逃生口是關掉 LearnWhile。另外，gate 作用在開發者送出的 prompt 上，而那可能是對 agent 權限或輸入請求的回覆，這擦到了 ADR-0001「LearnWhile 從不擋路」的承諾。

**決策。** 當 gate 啟用中且欠著一次 Review 時，即使開發者不在 Waiting，pane 也顯示那張欠著的卡片，讓 debt 隨時可以當場償還；任何評分都會清除它，pane 隨後回到 idle。Gate 作用在每一個送出的 prompt 上，沒有例外，包括對 agent 的回覆。ADR-0001 的字面保證仍然守住：agent 的請求從不被藏起，pane 也從不取得前景焦點。不過開發者可能得先完成一次 Review 才能回覆。被拒絕的方案：豁免對 agent 的回覆，那並不會移除死結（一個乾淨 `Stop` 之後的全新 prompt 也會在閒置時被擋，所以無論如何都需要 pay-while-idle），而且會削弱這個承諾。

**後果。** 當 gate 啟用中時，pane 不再是嚴格的「閒置即被動」：它會在 Waiting 之外浮出欠著的卡片。這侷限在選擇啟用的 gate 情境，是 Learning Contract 概念明確允許的，而 host 只有在這個 Session 已經看過一次 gate 查詢之後才進入這個狀態，所以從不傳 `--gate` 的開發者不會看到 idle pane 有任何改變。阻擋對 agent 的回覆是可能且被接受的；pay-while-idle 讓它不會死結。如果 dogfooding 顯示「沒有例外」的範圍咬得太痛，豁免回覆是第一個該重新考慮的洩壓閥。規格：[`docs/specs/prompt-gate.md`](../../specs/zh-TW/prompt-gate.md)。
