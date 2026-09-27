---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Hang line-end spaces past a right-to-left line's edge

A right-to-left paragraph whose line ends in left-to-right words, or the reverse, now aligns those words to the line's edge and leaves the space outside decorations and backgrounds, as Chrome does.
