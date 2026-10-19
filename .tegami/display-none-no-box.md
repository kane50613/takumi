---
packages:
  takumi-core: patch
---

# Keep a line whole around a `display: none` element

A `display: none` element inside a line of text, such as `Only <span style="display: none">sold out</span>$20`, no longer breaks the line in two. Such an element now gets no box at all, so `measure()` leaves it out of the children it returns.
