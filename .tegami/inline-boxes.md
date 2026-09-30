---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Lay out inline boxes as Chrome does

- A span's background, border and outline cover its own font's ascent and descent on every line, even when it holds only other spans or padding, instead of the whole `line-height`. A wrapped outline joins its lines only where they touch.
- A span's background and outline stop at the last glyph before a line break, like its text decoration.
- A span's left and right margins push the text beside it apart, on the parent's background, and a negative margin overlaps it with its neighbors.
- Each line grows to the block's own `line-height` and font, even when it holds only a span with a smaller line height. Under `line-height: normal`, a fallback font, such as an emoji font, grows the line by its own line spacing.
- A span with a larger font than its text makes its line as tall as Chrome does.
- `vertical-align` on an inline span moves its text, background and children, and grows the line to fit. `sub` and `super` shift by the parent's font size, percentages refer to the span's own line height, and `middle` uses the parent font's x-height.
