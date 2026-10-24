---
packages:
  takumi-core: minor
---

# Lay out SVG text as Chrome does

SVG `<text>` now takes its font, size, spacing, and anchoring from the CSS cascade and lays out with Chrome's SVG text algorithm. Text without a `font-size` is 16px, as in Chrome, instead of 12px, and text without a `font-family` uses the same default font as the rest of the page. Text in an SVG image that also uses a filter is not drawn until filters move to the same pipeline.
