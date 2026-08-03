//! Turn the `5mdld/anki-jlpt-decks` source export into LearnWhile seed TSVs.
//!
//! The upstream deck ships its source as a Tab-separated Anki text export
//! (`deck-source/notes.csv`), not just the built `.apkg`, so no SQLite unpacking
//! is needed. Every data row is exactly 39 tab fields with no CSV quoting, which
//! makes a line-based `split('\t')` parse safe and keeps this tool dependency-free.
//!
//! Usage:
//!     cargo run --example extract_anki_jlpt -- <notes.csv> <out-dir>
//!
//! Output: `n5.tsv` .. `n1.tsv` in `<out-dir>`, one `front<TAB>back` per line.
//!     front = VocabKanji                              e.g. 高校
//!     back  = <furigana>  <POS> <pitch> ・ <def-TC>   e.g. 高校[こうこう]  名 ⓪ ・ 高中
//!
//! The `[reading]` bracket (Anki furigana notation, rendered on the answer side
//! by the furigana feature) is added only when the word has kanji AND the reading
//! is pure kana. Katakana loanwords keep a latin etymology in the reading column,
//! so they stay bare. Backs are forced onto one line because the seed importer
//! (`parse_tsv`) is line-based.
//!
//! Source data is CC BY-NC 4.0 (© 5mdld/anki-jlpt-decks); see
//! `data/anki-jlpt/README.md`.

use std::path::Path;
use std::process::ExitCode;

// 0-based columns per the export header (#notetype 0, #deck 1, #tags 38).
const C_DECK: usize = 1;
const C_KANJI: usize = 3;
const C_PITCH: usize = 4;
const C_POS: usize = 5;
const C_FURI: usize = 6;
const C_DEF_TC: usize = 8;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let src = args.next().unwrap_or_else(|| "notes.csv".to_string());
    let out = args.next().unwrap_or_else(|| ".".to_string());

    let contents = match std::fs::read_to_string(&src) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("cannot read {src}: {e}");
            return ExitCode::FAILURE;
        }
    };

    // buckets[0] = N1 .. buckets[4] = N5
    let mut buckets: [Vec<String>; 5] = Default::default();
    let mut skipped = 0usize;

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
        buckets[(level - 1) as usize].push(format!("{front}\t{back}"));
    }

    for (i, lines) in buckets.iter().enumerate() {
        let level = i + 1; // buckets[0] holds N1 .. buckets[4] holds N5
        let path = Path::new(&out).join(format!("n{level}.tsv"));
        let body = if lines.is_empty() {
            String::new()
        } else {
            format!("{}\n", lines.join("\n"))
        };
        if let Err(e) = std::fs::write(&path, body) {
            eprintln!("cannot write {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
        println!("{}  {} cards (N{level})", path.display(), lines.len());
    }
    if skipped > 0 {
        println!("(skipped {skipped} rows: no level / empty word or def)");
    }
    ExitCode::SUCCESS
}

/// The JLPT level from a deck path like `eggrolls-JLPT10k-v3.5::1-N5::...`,
/// as the digit after the sole `-N`. Returns `None` if absent or out of 1..=5.
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
