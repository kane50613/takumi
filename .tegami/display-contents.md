---
packages:
  takumi-core: minor
  takumi-pdf: minor
---

# `display: contents` renders an element's children in its place

An element with `display: contents` generates no box, so its children and its `::before` and `::after` lay out as children of its parent, and still inherit from it. A flex or grid container takes those children as its items. Replaced elements and form controls render nothing with it, and the root element computes to `block`, as in browsers. The Tailwind `contents` class sets it.

Before, the value was not recognized and the declaration was dropped.
