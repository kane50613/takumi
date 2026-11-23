---
packages:
  takumi-core: minor
---

# `currentColor` in `color-mix()`, and system colours

`color-mix()` and relative colours accept `currentColor` and resolve it against the colour of the element that uses them, as Blink does. In `color` itself, `currentColor` is the parent's colour. A relative colour whose origin is `light-dark()` takes the origin of the element's used color scheme. System colours such as `Canvas`, `CanvasText`, `ButtonFace` and `LinkText` are supported, with Chromium's light and dark values.
