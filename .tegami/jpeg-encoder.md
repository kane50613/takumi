---
packages:
  "takumi": patch
---

# Write smaller JPEG files

JPEG output now uses optimized Huffman tables, which makes text and UI images about 29% smaller at the same quality. A photograph shrinks by about 4%. The encoder no longer copies the image to drop its alpha channel first.
