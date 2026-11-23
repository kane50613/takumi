---
packages:
  takumi-core: minor
---

# `light-dark()` follows `color-scheme`

`color-scheme` is supported, and `light-dark()` takes the colour of an element's used color scheme, as in a browser. That is the viewport's preferred scheme when `color-scheme` lists it, otherwise the one it lists, and light under `normal`. `color-scheme` changes only `light-dark()` and system colours, so the canvas, the default text colour, and form controls stay light.

```html
<div style="color-scheme: dark; background: light-dark(white, black)"></div>
```

```diff
- A white box
+ A black box
```
