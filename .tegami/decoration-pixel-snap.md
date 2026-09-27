---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Draw text decorations on whole pixels

Underlines, overlines, and line-throughs now round their top edge to the nearest pixel and their thickness down to a whole pixel, at least 1px, as Chrome does. They used to straddle pixel rows and paint a faint half-covered row above and below the line.
