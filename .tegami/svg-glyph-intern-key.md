---
packages:
  takumi-svg: patch
---

# Write text into SVG faster

A glyph that repeats is now looked up by its outline and scale, so its path is formatted once instead of at every occurrence. Text-heavy SVG output renders about 40–57% faster, with identical output.
