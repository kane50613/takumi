---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Lay out inline boxes as Chrome does

- A span's background, border and outline cover its font's ascent and descent, not the whole `line-height`.
- A span's background and outline stop at the last glyph before a line break.
- A span's side margins push the text beside it apart. Negative margins overlap it.
- Each line is at least the block's own `line-height`. Under `line-height: normal`, a fallback font such as emoji grows the line.
- A span with a larger font makes its line as tall as Chrome does.
- `vertical-align` on a span moves its text, background and children, and grows the line to fit.
- Text in a `sub` or `super` span no longer kerns against the text around it.
- A span's `background-image` paints as one strip across its lines, so gradient text inside a heading shows.
- `box-decoration-break: clone` repeats a span's border, padding and corners on every line.
- A `direction: rtl` span puts its start padding, border and margin on the right.
