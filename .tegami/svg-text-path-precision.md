---
packages:
  takumi-core: patch
---

# Text on a straight `textPath` lands closer to its place

Glyphs along an SVG `textPath` and strokes with `vector-effect: non-scaling-stroke` now sit closer to where Chromium draws them. A glyph along a straight segment no longer lands up to a tenth of a pixel off, so `<textPath>` along `M0,80h100` draws the same glyph as `<text x="0" y="80">`. A path with a segment of no length, such as `M0,80h0L100,80`, now places its glyphs instead of dropping them. A non-scaling stroke under a scaled viewBox drifts less, though some scales still leave a rounding error.
