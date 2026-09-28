---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Carry `text-decoration` into the boxes inside

A `text-decoration` now reaches the text of every in-flow box inside the element that sets it, as Chrome's does. A `<span>` inside an underlined block is underlined, and nested decorations all draw. Inline blocks, floats, absolutely positioned boxes and outside list markers still stop it.
