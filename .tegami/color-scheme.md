---
packages:
  takumi-core: minor
---

# `light-dark()` follows `color-scheme`

`color-scheme` is supported, and `light-dark()` takes the colour of an element's used color scheme: the viewport's preferred scheme when `color-scheme` lists it, otherwise the one it lists, and light under `normal`, as in a browser. A `light-dark()` inside `color-mix()` or a relative colour still takes its first colour.

```html
<div style="color-scheme: dark; background: light-dark(white, black)"></div>
```

```diff
- A white box
+ A black box
```
