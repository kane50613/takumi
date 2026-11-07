---
packages:
  takumi-core: patch
---

# `light-dark()` takes `currentColor`

`border: 4px solid light-dark(currentColor, blue)` used to make its declaration invalid, so the element drew no border. `light-dark()` now takes `currentColor` wherever a colour can be `currentColor`.
