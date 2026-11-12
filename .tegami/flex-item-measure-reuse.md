---
packages:
  takumi-core: patch
  takumi-pdf: patch
---

# A long table in a column flexbox renders with less time and memory

A column flexbox that broke across pages laid each item out in full once more before laying it out page by page, and kept every layout inside the item until the flexbox ended. It now reuses the item's measure for that pass, as Chromium does, and frees what the item holds as its pages are drawn. An 11,000-row table in a column flexbox peaks at 1.35 GB in place of 1.46 GB, with the same PDF.
