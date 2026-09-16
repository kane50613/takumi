---
packages:
  "takumi": minor
---

# Render text-heavy layouts faster

Shaped text is reused across measure, baseline and paint, and `text-decoration-skip-ink` skips glyphs that never reach the underline band. Output is unchanged; a 24-card page builds its paint tree in 9.5 ms instead of 22 ms.
