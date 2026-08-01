//! Japanese (kanji/kana) card content renders through the host boundary exactly as a real terminal
//! shows it. Wide graphemes occupy two columns; the pane text must read as contiguous kanji, not the
//! space-separated "日 本 語" that a naive cell-by-cell flatten produces. If this passes, a developer
//! seeding a Japanese deck sees a well-formed card, not a mangled one.

use crossterm::event::KeyCode;
use learnwhile::testing::spawn_test_host_with_cards;

const REVEAL: KeyCode = KeyCode::Char(' ');

const FRONT: &str = "「勉強」は英語で何と言いますか？";
const BACK: &str = "study（勉強する）";

// A furigana-annotated card: the reading rides inline in the front (spec §Notation).
const RUBY_FRONT: &str = " 勉強[べんきょう]";
const RUBY_READING: &str = "べんきょう";
const RUBY_BASE: &str = "勉強";
const RUBY_BACK: &str = "study";

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

#[test]
fn furigana_is_hidden_until_reveal_then_stacks_over_its_kanji() {
    let host = spawn_test_host_with_cards(&[(RUBY_FRONT, RUBY_BACK)]);
    host.open("session-a");

    // Question side: base kanji only. The reading is the answer, so it must not be on screen yet.
    host.wait_for(RUBY_BASE);
    let pane = host.pane();
    assert!(
        !pane.contains(RUBY_READING),
        "the reading leaked onto the question side. Pane:\n{pane}"
    );

    // Reveal: the reading now sits on the line directly above its kanji.
    host.press(REVEAL);
    host.wait_for(RUBY_READING);
    let pane = host.pane();
    let lines: Vec<&str> = pane.lines().collect();
    let base_row = lines
        .iter()
        .position(|l| l.contains(RUBY_BASE))
        .expect("base kanji on screen");
    assert!(
        base_row > 0 && lines[base_row - 1].contains(RUBY_READING),
        "the reading was not on the line directly above its kanji. Pane:\n{pane}"
    );

    host.shutdown();
}
