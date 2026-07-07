---
packages:
  npm:@takumi-rs/core:
    replay:
      - exit-prerelease(npm:@takumi-rs/core)
  npm:@takumi-rs/wasm:
    replay:
      - exit-prerelease(npm:@takumi-rs/wasm)
---

### Apply structured keyframes in `renderAnimation`

`renderAnimation` accepted a `keyframes` option but never registered it, so
structured keyframes animated with `render` yet stayed static in animations. It
now extends the stylesheet with them like the other entry points.
