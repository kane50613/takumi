---
packages:
  "takumi-core": patch
  "takumi": patch
---

# Lay out text faster

The glyph cache and per-render font metrics hash their keys with a faster hasher. Text-heavy layouts take about 6% less time.
