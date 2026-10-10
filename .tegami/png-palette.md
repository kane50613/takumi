---
packages:
  "takumi": patch
---

# Write PNGs of at most 256 colors with a palette

A PNG whose pixels take at most 256 colors, as most text and docs cards do, is now stored as one palette index per pixel. On real cards that makes the file about 37% smaller and encodes it faster. The pixels stay exactly the same.
