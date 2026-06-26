---
packages:
  "npm:takumi-js": patch
---

### Explain a failed native backend load

When `@takumi-rs/core` can't load on Node, the render now throws an error
pointing at the `module` option for the WASM backend, instead of surfacing the
raw native loader failure.
