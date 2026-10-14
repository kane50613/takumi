---
packages:
  "takumi-pdf": patch
---

# Keep words whole when a letter is drawn as a base and a mark

PDF text extraction no longer splits a word after a letter the font draws as a base glyph and a combining mark, such as `Ś` from a font that has only `S` and U+0301. Before, `pdftotext` read `Świnoujście` as `Ś winoujś cie`.
