---
packages:
  takumi-core: minor
  takumi: minor
---

# `color-mix()` and relative colours can read `currentColor`, and system colours render

These declarations used to be dropped as invalid and now render as they do in Chrome:

- `color-mix()` and relative colours with `currentColor`, which resolve against the colour of the element that uses them. In `color` itself, `currentColor` is the parent's colour.
- System colours such as `Canvas`, `CanvasText`, `ButtonFace` and `LinkText`, with Chromium's light and dark values.

A relative colour whose origin is `light-dark()` now takes the origin in the element's used color scheme instead of the light one.

`color-mix()` percentages follow Chrome too. A number or a percentage below 0% or above 100% makes the declaration invalid, and percentages that sum to zero mix to transparent.

#### Migration

`ColorInput` is no longer `Copy`, since `color-mix()` and relative colours keep their arguments, and neither are `Border`, `BoxShadow`, `TextShadow`, and `TextStroke`, which hold one. Clone a value you move twice.

```diff
- let fill = color;
+ let fill = color.clone();
  let stroke = color;
```
