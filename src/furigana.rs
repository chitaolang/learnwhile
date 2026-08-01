//! Furigana: parse Anki-compatible reading notation and lay it out for a terminal grid.
//!
//! A card author writes readings inline, e.g. ` 勉強[べんきょう]`: the square-bracketed kana is
//! the reading for the run of non-space characters immediately before the `[`, and a single space
//! delimits that base from earlier text (and is not shown). This mirrors Anki so decks stay
//! portable (spec §Notation, ADR-0012).
//!
//! The module offers exactly Anki's two relevant render modes. [`base_only`] is Anki's `kanji:`
//! (readings and delimiter spaces stripped) and drives the question side; [`layout`] is Anki's
//! `furigana:` (the reading stacked over its base) and drives the answer side (ADR-0013). Both are
//! pure: nothing here knows about a card, the host, or the database.

use ratatui::buffer::CellWidth;

/// One parsed span of a field: literal text, or a base with its reading.
enum Unit {
    Plain(String),
    Ruby { base: String, reading: String },
}

/// The display width of a string in terminal columns, via the same width source the renderer and
/// the test harness use (so the buffer and this layout always agree).
fn width_of(s: &str) -> usize {
    s.cell_width() as usize
}

/// Split a field into units. A `[reading]` attaches to the trailing run of non-space characters
/// before it, and the single delimiter space in front of that run (if any) is consumed. An
/// unclosed `[`, an empty `[]`, or a `[…]` with no base before it is left literal rather than
/// treated as markup, so a malformed annotation degrades to visible source instead of a panic.
fn tokenize(field: &str) -> Vec<Unit> {
    let chars: Vec<char> = field.chars().collect();
    let mut units = Vec::new();
    let mut pending = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '[' {
            if let Some(close) = (i + 1..chars.len()).find(|&j| chars[j] == ']') {
                let reading: String = chars[i + 1..close].iter().collect();
                let base_len = pending.chars().rev().take_while(|c| *c != ' ').count();
                let total = pending.chars().count();
                let base: String = pending.chars().skip(total - base_len).collect();
                if !reading.is_empty() && !base.is_empty() {
                    let mut head: String = pending.chars().take(total - base_len).collect();
                    // The delimiter space that scoped the base is not part of the shown text.
                    if head.ends_with(' ') {
                        head.pop();
                    }
                    if !head.is_empty() {
                        units.push(Unit::Plain(head));
                    }
                    pending.clear();
                    units.push(Unit::Ruby { base, reading });
                    i = close + 1;
                    continue;
                }
            }
            pending.push('[');
            i += 1;
        } else {
            pending.push(c);
            i += 1;
        }
    }
    if !pending.is_empty() {
        units.push(Unit::Plain(pending));
    }
    units
}

/// Whether a field carries at least one reading. Cheap gate the renderer uses to keep every
/// unannotated card on its existing, unchanged draw path.
pub fn has_ruby(field: &str) -> bool {
    tokenize(field)
        .iter()
        .any(|u| matches!(u, Unit::Ruby { .. }))
}

/// The base text with every reading and delimiter space removed (Anki's `kanji:`). ` 勉強[べんきょう]`
/// becomes `勉強`. A field with no annotation returns unchanged.
pub fn base_only(field: &str) -> String {
    let mut out = String::new();
    for unit in tokenize(field) {
        match unit {
            Unit::Plain(text) => out.push_str(&text),
            Unit::Ruby { base, .. } => out.push_str(&base),
        }
    }
    out
}

/// A single terminal column-group: a base and the reading centered over it, sized to the wider.
struct Column {
    base: String,
    reading: String,
    cols: usize,
}

/// Pad `text` to `cols` display columns, centering it (extra space biased to the right).
fn center(text: &str, cols: usize) -> String {
    let pad = cols.saturating_sub(width_of(text));
    let left = pad / 2;
    let right = pad - left;
    format!("{}{}{}", " ".repeat(left), text, " ".repeat(right))
}

/// Char-level greedy wrap by display width, for plain (reading-free) content on the stacked path.
/// Unannotated cards never reach here; they keep ratatui's word wrap.
fn wrap_plain(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    let mut lines = Vec::new();
    let mut cur = String::new();
    let mut cur_w = 0;
    let mut buf = [0u8; 4];
    for ch in text.chars() {
        let w = width_of(ch.encode_utf8(&mut buf));
        if cur_w + w > width && !cur.is_empty() {
            lines.push(std::mem::take(&mut cur));
            cur_w = 0;
        }
        cur.push(ch);
        cur_w += w;
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Render `field` as display lines that fit `width` columns. With no annotation, one wrapped line
/// per row (Anki's plain text). With annotations, pairs of (reading line, base line) that wrap by
/// whole column-groups so a reading never separates from its base, blank-line-separated for
/// legibility (spec §Stacked layout algorithm).
pub fn layout(field: &str, width: usize) -> Vec<String> {
    let units = tokenize(field);
    if !units.iter().any(|u| matches!(u, Unit::Ruby { .. })) {
        let text: String = units
            .iter()
            .map(|u| match u {
                Unit::Plain(t) => t.as_str(),
                Unit::Ruby { .. } => "",
            })
            .collect();
        return wrap_plain(&text, width);
    }

    // Ruby units are one column-group each; plain runs break into per-character groups so they
    // wrap cleanly and sit under a blank reading.
    let mut columns: Vec<Column> = Vec::new();
    for unit in units {
        match unit {
            Unit::Ruby { base, reading } => {
                let cols = width_of(&base).max(width_of(&reading));
                columns.push(Column {
                    base,
                    reading,
                    cols,
                });
            }
            Unit::Plain(text) => {
                let mut buf = [0u8; 4];
                for ch in text.chars() {
                    let s = ch.encode_utf8(&mut buf);
                    columns.push(Column {
                        base: s.to_string(),
                        reading: String::new(),
                        cols: width_of(s),
                    });
                }
            }
        }
    }

    let mut lines = Vec::new();
    let mut group: Vec<&Column> = Vec::new();
    let mut used = 0;
    for col in &columns {
        if !group.is_empty() && used + col.cols > width {
            flush_group(&group, &mut lines);
            lines.push(String::new());
            group.clear();
            used = 0;
        }
        group.push(col);
        used += col.cols;
    }
    if !group.is_empty() {
        flush_group(&group, &mut lines);
    }
    lines
}

/// Emit one column-group as a reading line above a base line.
fn flush_group(group: &[&Column], lines: &mut Vec<String>) {
    let mut reading = String::new();
    let mut base = String::new();
    for col in group {
        reading.push_str(&center(&col.reading, col.cols));
        base.push_str(&center(&col.base, col.cols));
    }
    lines.push(reading.trim_end().to_string());
    lines.push(base.trim_end().to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_is_unchanged() {
        assert!(!has_ruby("What does FSRS stand for?"));
        assert_eq!(base_only("hello world"), "hello world");
        assert_eq!(layout("hello", 40), vec!["hello"]);
    }

    #[test]
    fn base_only_strips_reading_and_delimiter_space() {
        assert_eq!(base_only(" 勉強[べんきょう]"), "勉強");
        assert_eq!(base_only("この 間[あいだ]"), "この間");
        assert_eq!(base_only("食[た]べる"), "食べる");
    }

    #[test]
    fn a_reading_stacks_centered_over_its_base() {
        // 勉強 is 4 columns, べんきょう is 10, so the group is 10 wide and 勉強 is centered.
        let out = layout(" 勉強[べんきょう]", 40);
        assert_eq!(out, vec!["べんきょう", "   勉強"]);
    }

    #[test]
    fn okurigana_leaves_trailing_kana_plain() {
        // た over 食, then べる with a blank reading above.
        assert_eq!(layout("食[た]べる", 40), vec!["た", "食べる"]);
    }

    #[test]
    fn adjacent_words_each_keep_their_own_reading() {
        // にほん (6 cols) covers 日本 (4 cols, centered), ご covers 語. Each reading stays over
        // its own base.
        assert_eq!(
            layout("日本[にほん]語[ご]", 40),
            vec!["にほんご", " 日本 語"]
        );
    }

    #[test]
    fn a_malformed_annotation_is_left_literal() {
        assert!(!has_ruby("cost is 100[ dollars"));
        assert_eq!(base_only("cost is 100[ dollars"), "cost is 100[ dollars");
        assert_eq!(base_only("empty[]"), "empty[]");
    }

    #[test]
    fn a_ruby_group_never_splits_across_a_wrap() {
        // Width 6 fits one 勉強[べんきょう] group (10 cols alone) per line, blank-separated.
        let out = layout(" 勉強[べんきょう] 勉強[べんきょう]", 10);
        assert_eq!(
            out,
            vec!["べんきょう", "   勉強", "", "べんきょう", "   勉強"]
        );
    }
}
