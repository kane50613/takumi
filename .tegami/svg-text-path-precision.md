---
packages:
  takumi-core: patch
---

# Text on a straight `textPath` sits exactly where it should

A glyph set along a straight `textPath` segment no longer lands up to a tenth of a pixel off its place. A straight segment is now measured exactly, as Chromium's `SkContourMeasure` measures it, so `<textPath>` along `M0,80h100` draws the same glyph as `<text x="0" y="80">`. A `textPath` whose path has a segment of no length, such as the `h0` in `M0,80h0L100,80`, now places its glyphs instead of dropping them.

A `vector-effect: non-scaling-stroke` stroke also no longer drifts by a fraction of a pixel under a scaled viewBox, because the inverse transform is computed in double precision as in Blink.
