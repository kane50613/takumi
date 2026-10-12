---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Fit text with `text-fit` as Chrome does

- Fixed `letter-spacing`, `word-spacing` and `line-height` keep their size when a line scales.
- Text strokes, shadows and decorations scale with the line.
- A line within 2px of its box stays unscaled, and so does one that spacing already fills.
- `grow` limits under 100% and `shrink` limits over 100% stop scaling.
- Indented, centered, right-aligned and right-to-left lines land where Chrome puts them, on the same baseline.
- Span offsets and backgrounds no longer scale twice.
- With `text-overflow: ellipsis`, a line that fits once scaled gets no ellipsis.
