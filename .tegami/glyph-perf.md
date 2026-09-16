---
packages:
  "takumi": minor
---

# Render repeated glyphs faster

Each glyph run is walked once, and `text-decoration-skip-ink` intercepts and `line-height: normal` metrics are reused within a render. Output is unchanged.
