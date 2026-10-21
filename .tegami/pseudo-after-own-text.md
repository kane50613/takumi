---
packages:
  takumi-core: patch
---

# Lay out a flex or grid container's text as an item beside its `::after`

An element with its own text and an `::after` now lays the text out as an anonymous flex or grid item, as it already did beside a `::before`. Before, the text sat on the container itself, so `align-items` and `justify-content` ignored it and the `::after` box landed on top of it.
