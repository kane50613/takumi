---
packages:
  takumi-core: patch
---

# Relative colours over a powerless origin

`hsl(from white 120 50 50)` now keeps the literal hue and resolves to the same green as in Chrome, where it had dropped the hue to zero.
