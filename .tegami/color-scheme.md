---
packages:
  takumi-core: minor
---

# `light-dark()` and system colours follow `color-scheme`

`light-dark()` and system colours take the colour of an element's used color scheme, as in a browser. The used scheme is:

- the viewport's preferred scheme, when `color-scheme` lists it
- otherwise the scheme `color-scheme` lists
- light, under `normal`

The canvas, the default text colour, and form controls stay light.

```html
<div style="color-scheme: dark; background: light-dark(white, black)"></div>
```

```diff
- A white box
+ A black box
```
