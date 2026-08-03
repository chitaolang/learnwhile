# Anki JLPT seed decks

LearnWhile seed TSVs derived from the [`5mdld/anki-jlpt-decks`](https://github.com/5mdld/anki-jlpt-decks)
JLPT 10k vocabulary deck (`eggrolls-JLPT10k`). One file per JLPT level.

| File | Level | Cards |
|------|-------|------:|
| `n5.tsv` | N5 |   807 |
| `n4.tsv` | N4 |   757 |
| `n3.tsv` | N3 | 1,818 |
| `n2.tsv` | N2 | 3,208 |
| `n1.tsv` | N1 | 4,044 |

Each line is `front<TAB>back`:

```
高校	高校[こうこう]  名 ⓪ ・ 高中
グラム	グラム  名 ①⓪ ・ 克，公克
```

- **front** = the vocabulary word (question side).
- **back** = furigana notation + part of speech + pitch accent + `・` + traditional-Chinese
  definition, on a single line. The `word[reading]` bracket is Anki furigana notation, rendered
  stacked on the answer side; it is present only when the word has kanji and the reading is pure
  kana, so katakana loanwords (whose reading column holds a latin etymology) stay bare.

Seed a level with:

```
learnwhile seed data/anki-jlpt/n5.tsv
```

## Regenerating

These files are generated from the upstream Tab-separated source export by
`examples/extract_anki_jlpt.rs`:

```
curl -sL https://raw.githubusercontent.com/5mdld/anki-jlpt-decks/main/deck-source/notes.csv -o notes.csv
cargo run --example extract_anki_jlpt -- notes.csv data/anki-jlpt
```

## License and attribution

Source data © [`5mdld/anki-jlpt-decks`](https://github.com/5mdld/anki-jlpt-decks), licensed
**CC BY-NC 4.0**. These derived TSVs carry the same license: attribution required, non-commercial
use only.
