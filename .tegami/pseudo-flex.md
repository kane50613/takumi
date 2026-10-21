---
packages:
  takumi-core: patch
---

# Lay out `::before` and `::after` as flex and grid containers

A `::before` or `::after` with `display: flex` or `display: grid` now lays its content out as items, so `align-items`, `justify-content`, and `place-items` center a generated number or icon. Before, the box became a block and ignored them. Adjacent strings and `attr()` values in `content` form one item, as in Chrome.
