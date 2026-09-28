---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Draw text decorations as Chrome does

- Underlines, overlines and line-throughs round their top edge to the nearest pixel and their thickness down to a whole pixel, at least 1px.
- PDF applies a span's `opacity` to its text decorations.
- An overline rests on top of the text. A line-through sits a third of the ascent above the baseline instead of at the font's strikeout position.
- `text-underline-position: auto` puts the underline half its thickness, at least 1px, below the baseline. `from-font` keeps the font's underline position, `under` leaves a pixel below the em box, and a set `text-underline-offset` drops the `auto` gap.
- `text-decoration-thickness: from-font` uses the font's underline thickness for every line.
- `text-decoration: overline 12px red` keeps a thickness written right after the line keyword.
