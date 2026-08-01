# furigana 是行內標記，於繪製時才解析與渲染

**背景。** 日文卡片的正面常常是學習者正在習得讀音的漢字，而那個讀音會想以 ruby（振假名／furigana）的形式標在漢字上方。LearnWhile 把卡片存成 `front`/`back` 這對不透明的 UTF-8 欄位，而每個子系統（seed、hash、選卡、FSRS、Review）都把那段文字當成從不解讀的資料。讀音必須綁定到欄位*內部*特定的漢字，而單一欄位可能有好幾段各自帶讀音的漢字，所以「一張卡一個讀音」無法表達這件事。

**決策。** 讀音以 Anki 相容的 furigana 標記（`base[reading]`，以空白分隔）行內寫在既有的 `front`/`back` 文字裡，只在 render 時由 Renderer 與一個純粹的 furigana module 解析。存下來的是作者輸入的原始字串，含標記在內。拒絕的方案：另設一個 `reading` 欄位，它無法表達單一欄位中多段各自帶讀音的漢字，且會為了純視覺的需求強迫做 schema migration。拒絕的方案：在 ingest 時自動產生讀音（MeCab／kakasi），它對人名與罕見讀音是有損的。

**後果。** 沒有 schema 變更、沒有 migration，卡片模型、seeding、`content_hash`、選卡與 FSRS 全都不動。標記以普通文字隨行，所以 `content_hash` 會把 `勉強` 與 ` 勉強[べんきょう]` 視為不同的卡片，這是對的：它們是不同的提示。`learnwhile cards` 列出的是含標記的原始 front，這正是在除錯讀音的作者想看到的。Deck 仍可與 Anki 雙向互通。所有新邏輯都落在 `renderer.rs` 加 `src/furigana.rs`；系統中其他任何部分都不知道這個標記存在。規格：[`docs/specs/furigana-ruby-display.md`](../../specs/zh-TW/furigana-ruby-display.md)。
