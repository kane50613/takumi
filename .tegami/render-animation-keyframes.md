---
packages:
  "npm:@takumi-rs/core": patch
  "npm:@takumi-rs/wasm": patch
---

### Apply structured keyframes in `renderAnimation`

`renderAnimation` accepted a `keyframes` option but never registered it, so
structured keyframes animated with `render` yet stayed static in animations. It
now extends the stylesheet with them like the other entry points.
