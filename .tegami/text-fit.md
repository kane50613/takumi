---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Fit text with `text-fit` as Chrome does

- Fixed `letter-spacing` and `word-spacing` keep their size when `text-fit` scales a line, and a fixed `line-height` keeps its height around the scaled glyphs.
- `-webkit-text-stroke`, `text-shadow` and text decorations scale with the line.
- A line within 2px of its box stays unscaled, counting its `text-indent` under `per-line` and `per-line-all`. A `grow` limit under 100% or a `shrink` limit over 100% stops the text from scaling.
- A line keeps its `text-indent`, and `center`, `right` and right-to-left lines land where Chrome puts them.
- Spans keep their `vertical-align` offsets and backgrounds instead of scaling them a second time.
- Text sits on the baseline Chrome paints it at, and its glyphs snap to the same pixel rows.
- A line whose fixed `letter-spacing` or `word-spacing` already fills its box stays unscaled, as in Chrome, where it used to vanish.
