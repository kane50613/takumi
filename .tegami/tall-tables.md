---
packages:
  "takumi-core": patch
  "takumi": patch
  "takumi-pdf": patch
---

# Lay out tables with more than 10,000 rows

Rows past the 10,000th used to pile up on top of each other, without an error. A taller table now lays out every row, and a tagged PDF keeps every row in its structure tree.
