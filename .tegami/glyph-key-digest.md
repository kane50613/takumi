---
packages:
  "takumi-core": patch
  "takumi": patch
  "takumi-pdf": patch
---

# Look up glyphs faster

Each glyph's cache key now hashes 12 bytes instead of copying a 600-byte hasher. Large documents lay out about 2% faster.
