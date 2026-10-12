---
packages:
  "takumi-core": patch
  "takumi": patch
  "takumi-pdf": patch
---

# Lay out Latin text faster

Text below U+02C7, such as Latin, no longer scans the CJK tables to decide whether underlines skip its ink. Large documents lay out about 4% faster.
