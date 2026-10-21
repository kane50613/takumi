---
packages:
  takumi-core: patch
---

# Align and decorate SVG text with the font metrics HTML text uses

SVG `<text>` now places `dominant-baseline`, `alignment-baseline`, and text decorations from the same font metrics as HTML text. `baseline-shift="sub"` and `"super"` move text by half the font's height, as Chromium does, and decoration lines are a tenth of the font size thick.
