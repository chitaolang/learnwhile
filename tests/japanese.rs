//! Japanese (kanji/kana) card content renders through the host boundary exactly as a real terminal
//! shows it. Wide graphemes occupy two columns; the pane text must read as contiguous kanji, not the
//! space-separated "日 本 語" that a naive cell-by-cell flatten produces. If this passes, a developer
//! seeding a Japanese deck sees a well-formed card, not a mangled one.

use crossterm::event::KeyCode;
use learnwhile::testing::spawn_test_host_with_cards;

const REVEAL: KeyCode = KeyCode::Char(' ');

const FRONT: &str = "「勉強」は英語で何と言いますか？";
const BACK: &str = "study（勉強する）";

#[test]
fn a_kanji_card_renders_front_and_back_contiguously() {
    let host = spawn_test_host_with_cards(&[(FRONT, BACK)]);
    host.open("session-a");

    // The kanji front appears verbatim, with no columns injected between wide characters.
    host.wait_for(FRONT);
    let pane = host.pane();
    assert!(
        !pane.contains("勉 強"),
        "kanji were split across columns, not rendered as one word. Pane:\n{pane}"
    );

    // The answer, which mixes ASCII and kanji, is hidden until reveal and then shows intact.
    assert!(
        !pane.contains(BACK),
        "the answer was visible before reveal. Pane:\n{pane}"
    );
    host.press(REVEAL);
    host.wait_for(BACK);

    host.shutdown();
}
