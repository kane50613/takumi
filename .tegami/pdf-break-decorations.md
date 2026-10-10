---
packages:
  "takumi-pdf": patch
---

# Keep a box's border and padding with its first and last line across a page break

A page break no longer falls inside a box's top or bottom border and padding while an earlier break fits, as in Chrome. A table row whose cell padding does not fit now moves to the next page whole, instead of leaving its bottom border behind.
