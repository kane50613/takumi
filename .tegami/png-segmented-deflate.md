---
packages:
  "takumi": patch
---

# Encode PNG on several threads

A large flat-art PNG, the kind most cards and social images are, now deflates in up to eight segments at once. A 1200×630 card encodes about 2.5 times as fast. The file stays within a few hundred bytes of before, and every machine writes the same bytes.
