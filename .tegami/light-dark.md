---
packages:
  takumi-core: patch
---

# Parse `light-dark()` colors

`light-dark()` used to make its declaration invalid, so `border: 1px solid light-dark(red, blue)` drew no border and `border-color: light-dark(red, blue)` fell back to `currentColor`. It now takes its first color, since a render has no `color-scheme` and is light.
