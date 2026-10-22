---
packages:
  takumi-core: minor
---

# Stack boxes inside inline blocks and inline floats as Chrome does

A positioned box or a `z-index` inside an inline block or a float of a line now stacks with the rest of the page, as Chrome stacks it, and `measure` reports what an inline block holds.
