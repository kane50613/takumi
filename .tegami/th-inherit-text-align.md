---
packages:
  takumi-core: patch
---

# Align `th` cells with the `text-align` of their row

A `th` cell is centered only when its row's `text-align` is the initial value, as in a browser. Under a row or table with `text-align: right`, `left`, `end`, or `justify`, the cell now takes that alignment instead of staying centered.
