---
packages:
  "takumi-core": patch
  "takumi": patch
  "takumi-pdf": patch
---

# Lay out inline spans faster

An inline span works out its strut from its line height alone, without building its whole text style. Large documents with many spans lay out about 5% faster.
