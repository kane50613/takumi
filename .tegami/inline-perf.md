---
packages:
  "takumi": minor
---

# Render text-heavy layouts faster

Shaped text is reused across measurement, baseline lookup, and painting. `text-decoration-skip-ink` skips glyphs outside the underline band.
