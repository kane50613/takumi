---
packages:
  takumi-core: minor
---

# Support `@container` and `container-type`

`container-type`, `container-name`, and the `container` shorthand make a box a query container, and `@container` rules read its size, as in Chrome. A `size` or `inline-size` container takes no size from its content on the axes it contains. A query asks the nearest container that has its name and contains every axis it reads, and it matches nothing without one. Size features take `width`, `height`, `inline-size`, `block-size`, `aspect-ratio`, and `orientation`, with range syntax, and an `em` in a query reads the viewport's font size. `@container` inside a style rule is not read yet.
