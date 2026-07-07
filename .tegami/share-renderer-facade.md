---
packages:
  npm:@takumi-rs/helpers:
    type: minor
  npm:@takumi-rs/core:
    type: minor
  npm:@takumi-rs/wasm:
    type: minor
---

### Share the renderer facade between the napi and wasm bindings

The napi and wasm `Renderer` wrappers now build on a shared `prepareRenderInput`
helper in `@takumi-rs/helpers/renderer`, so their option types and render bodies
live in one place. Both backends check `signal` before and after resolving
fonts and images: on native, an already-aborted signal now throws before any
resource fetch.
