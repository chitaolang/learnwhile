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

These files are generated from the upstream Tab-separated source export by the `extract`
subcommand (backed by `src/anki.rs`):

```
curl -sL https://raw.githubusercontent.com/5mdld/anki-jlpt-decks/main/deck-source/notes.csv -o notes.csv
learnwhile extract notes.csv data/anki-jlpt
```

## License and attribution

The card data is derived from **【egg rolls】JLPT N1～N5 一万词 v3.5** (the `eggrolls-JLPT10k`
deck), created by **egg rolls** and published at
<https://github.com/5mdld/anki-jlpt-decks>. It is licensed under
[Creative Commons Attribution-NonCommercial 4.0 International (CC BY-NC 4.0)](https://creativecommons.org/licenses/by-nc/4.0/).
The upstream author's terms require crediting **egg rolls** as the original author and linking the
source repository.

**Modifications.** The upstream Tab-separated source export was transformed into these per-level
`front<TAB>back` TSVs by `learnwhile extract`: only the vocabulary word, kana reading, pitch accent,
part of speech, and traditional-Chinese definition are kept and reformatted as Anki furigana
notation. The audio, example sentences, and simplified-Chinese fields are dropped.

These derived files carry the same **CC BY-NC 4.0** license. Keep this attribution to **egg rolls**
and the [source repository](https://github.com/5mdld/anki-jlpt-decks), and use them for
non-commercial purposes only.
