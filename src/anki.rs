//! Convert the `5mdld/anki-jlpt-decks` source export into LearnWhile seed rows.
//!
//! The upstream deck ships its source as a Tab-separated Anki text export
//! (`deck-source/notes.csv`), not just the built `.apkg`, so no SQLite unpacking
//! is needed. Every data row is exactly 39 tab fields with no CSV quoting, which
//! makes a line-based `split('\t')` parse safe and keeps this dependency-free.
//!
//! The output is the `front\tback` shape that `seed` ingests (see `parse_tsv`):
//!     front = VocabKanji                              e.g. 高校
//!     back  = <furigana>  <POS> <pitch> ・ <def-TC>   e.g. 高校[こうこう]  名 ⓪ ・ 高中
//!
//! The `[reading]` bracket is Anki furigana notation, rendered stacked on the
//! answer side by `furigana`. It is added only when the word has kanji AND the
//! reading is pure kana; katakana loanwords keep a latin etymology in the reading
//! column, so they stay bare. Backs are one line because `parse_tsv` is line-based.
//!
//! Source data is © egg rolls (https://github.com/5mdld/anki-jlpt-decks), CC BY-NC 4.0; see
//! `data/anki-jlpt/README.md`.

// 0-based columns per the export header (#notetype 0, #deck 1, #tags 38).
const C_DECK: usize = 1;
const C_KANJI: usize = 3;
const C_PITCH: usize = 4;
const C_POS: usize = 5;
const C_FURI: usize = 6;
const C_DEF_TC: usize = 8;

/// The five JLPT levels, N1 through N5, are returned as `[Vec<String>; 5]` where
/// index 0 is N1. Each `String` is one `front\tback` seed row.
pub const LEVELS: usize = 5;

/// Split the export into per-level seed rows. Header lines (`#…`), blank lines,
/// short rows, and rows missing a level, word, or definition are dropped so one
/// malformed row never aborts the extract; the count of dropped rows is returned.
pub fn extract(contents: &str) -> (Buckets, usize) {
    let mut buckets: Buckets = Default::default();
    let mut skipped = 0;

    for line in contents.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() <= C_DEF_TC {
            skipped += 1;
            continue;
        }
        let level = level_of(fields[C_DECK]);
        let front = clean(fields[C_KANJI]);
        let def_tc = clean(fields[C_DEF_TC]);
        let (Some(level), false, false) = (level, front.is_empty(), def_tc.is_empty()) else {
            skipped += 1;
            continue;
        };
        let back = back_of(
            &front,
            &clean(fields[C_FURI]),
            &clean(fields[C_POS]),
            &clean(fields[C_PITCH]),
            &def_tc,
        );
        buckets.rows[(level - 1) as usize].push(format!("{front}\t{back}"));
    }

    (buckets, skipped)
}

/// Per-level seed rows, `rows[0]` = N1 .. `rows[4]` = N5.
#[derive(Default)]
pub struct Buckets {
    pub rows: [Vec<String>; LEVELS],
}

/// The JLPT level from a deck path like `eggrolls-JLPT10k-v3.5::1-N5::...`, as the
/// digit after the sole `-N`. Returns `None` if absent or out of 1..=5.
fn level_of(deck: &str) -> Option<u8> {
    let idx = deck.find("-N")?;
    let d = deck[idx + 2..].chars().next()?.to_digit(10)? as u8;
    (1..=5).contains(&d).then_some(d)
}

/// Strip HTML tags (the export is `html:true`) and collapse all whitespace,
/// including embedded newlines, to single spaces.
fn clean(field: &str) -> String {
    let mut out = String::with_capacity(field.len());
    let mut in_tag = false;
    for c in field.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A reading is a real furigana reading only if every char is kana (or a joining
/// mark). Loanword "readings" hold a latin etymology and must not be bracketed.
fn is_kana_reading(furi: &str) -> bool {
    !furi.is_empty()
        && furi.chars().all(|c| {
            matches!(c,
                '\u{3040}'..='\u{309F}'   // hiragana
                | '\u{30A0}'..='\u{30FF}' // katakana (incl. ・ and ー)
                | '〜' | '～' | ' ')
        })
}

fn back_of(kanji: &str, furi: &str, pos: &str, pitch: &str, def_tc: &str) -> String {
    let head = if kanji != furi && is_kana_reading(furi) && !kanji.contains(' ') {
        format!("{kanji}[{furi}]")
    } else {
        kanji.to_string()
    };
    let meta = [pos, pitch]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let tail = [meta.as_str(), def_tc]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ・ ");
    if tail.is_empty() {
        head
    } else {
        format!("{head}  {tail}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build one 39-field export row with the columns this module reads set.
    fn row(deck: &str, kanji: &str, pitch: &str, pos: &str, furi: &str, def_tc: &str) -> String {
        let mut f = vec![""; 39];
        f[C_DECK] = deck;
        f[C_KANJI] = kanji;
        f[C_PITCH] = pitch;
        f[C_POS] = pos;
        f[C_FURI] = furi;
        f[C_DEF_TC] = def_tc;
        f.join("\t")
    }

    #[test]
    fn a_kanji_word_gets_a_furigana_bracket() {
        let line = row("d::1-N5::x", "高校", "⓪", "名", "こうこう", "高中");
        let (b, skipped) = extract(&line);
        assert_eq!(skipped, 0);
        assert_eq!(b.rows[4], vec!["高校\t高校[こうこう]  名 ⓪ ・ 高中"]);
    }

    #[test]
    fn a_loanword_reading_is_not_bracketed() {
        // The reading column holds a latin etymology, not a kana reading.
        let line = row(
            "d::1-N5::x",
            "グラム",
            "①⓪",
            "名",
            "gram；(法) gramme",
            "克，公克",
        );
        let (b, _) = extract(&line);
        assert_eq!(b.rows[4], vec!["グラム\tグラム  名 ①⓪ ・ 克，公克"]);
    }

    #[test]
    fn a_pure_kana_word_stays_bare() {
        let line = row("d::3-N3::x", "なす", "①", "名", "なす", "茄子");
        let (b, _) = extract(&line);
        assert_eq!(b.rows[2], vec!["なす\tなす  名 ① ・ 茄子"]);
    }

    #[test]
    fn html_tags_are_stripped_from_fields() {
        let line = row(
            "d::4-N2::x",
            "二酸化炭素",
            "",
            "名",
            "にさんかたんそ",
            "二氧化碳，CO<sub>2</sub>",
        );
        let (b, _) = extract(&line);
        assert_eq!(
            b.rows[1],
            vec!["二酸化炭素\t二酸化炭素[にさんかたんそ]  名 ・ 二氧化碳，CO2"]
        );
    }

    #[test]
    fn levels_route_to_their_bucket_and_headers_and_junk_are_skipped() {
        let text = format!(
            "#separator:Tab\n\n{}\n{}\nno-tabs-here\n",
            row("d::1-N5::x", "一", "", "", "いち", "一"),
            row("d::5-N1::x", "二", "", "", "に", "二"),
        );
        let (b, skipped) = extract(&text);
        assert_eq!(b.rows[4].len(), 1, "N5");
        assert_eq!(b.rows[0].len(), 1, "N1");
        assert_eq!(skipped, 1, "the tabless line");
    }
}
