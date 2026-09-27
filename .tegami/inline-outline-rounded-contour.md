---
packages:
  "takumi": patch
---

# Round a wrapped inline outline's corners

A `solid` or `double` outline around an inline element that wraps now rounds its corners by the element's `border-radius`, as Chrome does. Before, only a one-line outline was rounded.
