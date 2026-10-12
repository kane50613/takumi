---
packages:
  "takumi-core": patch
  "takumi": patch
  "takumi-pdf": patch
---

# Expand each font stack once per render

A `font-family` stack is matched against registered font subsets once per render, not on every text style. Large documents lay out about 4% faster.
