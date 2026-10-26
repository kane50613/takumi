---
packages:
  takumi-raster: patch
---

# Clip filtered output after applying filters

Ancestor overflow clips and an element's `clip-path` and `mask-image` now apply after filtering, preserving source pixels that contribute to blur and drop-shadow results. Clips inside the filtered element still constrain its contents.
