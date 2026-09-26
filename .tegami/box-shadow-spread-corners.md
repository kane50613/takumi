---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Keep square corners square when a `box-shadow` spreads

A spread shadow on a square box used to get rounded corners as wide as the spread. Its corners now follow the CSS outset-adjusted border radius: a square corner stays square, and a small radius grows less than the spread, as in Chrome.
