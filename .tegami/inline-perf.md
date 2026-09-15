---
packages:
  "takumi": minor
---

# Reuse shaped text across measure, baseline and paint

The render-local shape cache never matched: every paragraph starts with a direction mark, and the cache only accepted spans made of plain text. Direction marks are now cacheable, a shape is retained the first time it is seen, the inline walk resolves line metrics once instead of twice, and `text-decoration-skip-ink` skips glyphs whose outline never reaches the underline band. Output is unchanged; the paint tree of a 24-card page builds in 9.5 ms instead of 22 ms and the raster render in 26 ms instead of 40 ms.
