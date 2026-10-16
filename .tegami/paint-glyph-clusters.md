---
packages:
  "takumi-paint": minor
  "takumi-core": minor
---

# Map each glyph of a paint tree back to its text

A `Glyph` now has `cluster`: where its cluster starts in the run's `text`, in UTF-16 code units. Glyphs of one cluster share it, so a renderer can split a run into words or animate it a letter at a time without guessing from glyph positions.
