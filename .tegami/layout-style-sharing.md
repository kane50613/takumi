---
packages:
  takumi-core:
    type: patch
  takumi-pdf:
    type: patch
---

# Long documents share more of their layout styles

Boxes with equal layout styles now share one copy across a longer run of the document, so a table that repeats a row's styles keeps one copy of each. An 11,000-row PDF peaks at 939 MB of heap in place of 1,073 MB, with the same bytes.
