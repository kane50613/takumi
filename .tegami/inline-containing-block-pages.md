---
packages:
  takumi-core: patch
---

# Break boxes inside a positioned inline box across pages

In paged output, an absolutely positioned box whose containing block is a `position: relative` inline box now moves to the next page when it starts past the page end, and breaks across pages when it is taller than the space left.
