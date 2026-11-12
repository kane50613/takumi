---
packages:
  takumi-core: minor
---

# An inline element's background and border are a box in the paint tree

An element with `display: inline` and a `background` or `border` now has a box node in the paint tree, one for each line it sits on, with the element and its `InlineBackground` and `Border` drawables. Before, those drawables were part of the enclosing text node and no box named the element, where a `display: inline-block` one had a box.
