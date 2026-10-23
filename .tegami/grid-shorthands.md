---
packages:
  takumi-core: minor
---

# Support the `grid` and `grid-template` shorthands

`grid-template` and `grid` now set the grid longhands as browsers do, including the areas form such as `grid-template: "head head" 40px "side main" 1fr / 120px 1fr`. Track sizes also accept `min-content`, `max-content`, and `fit-content()`. A `grid-template-areas` row must now be a quoted string, and a value whose named areas do not form rectangles is rejected, as in browsers.
