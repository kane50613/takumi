---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Place overlines and line-throughs as Chrome does

An overline now rests on top of the text instead of inside it, and a line-through sits centred a third of the ascent above the baseline instead of at the font's strikeout position. `text-decoration-thickness: from-font` uses the font's underline thickness for every line.
