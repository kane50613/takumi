---
packages:
  "takumi": patch
---

# Encode photographic PNGs on several threads

A PNG that `pick` treats as a photograph now filters and deflates in segments at once too, so a 1200×630 photo encodes in about 6ms instead of 18ms.
