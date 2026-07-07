---
packages:
  npm:@takumi-rs/helpers:
    replay:
      - exit-prerelease(npm:@takumi-rs/helpers)
  npm:@takumi-rs/core:
    replay:
      - exit-prerelease(npm:@takumi-rs/core)
  npm:@takumi-rs/wasm:
    replay:
      - exit-prerelease(npm:@takumi-rs/wasm)
---

### Share the renderer facade between the napi and wasm bindings

The napi and wasm `Renderer` wrappers now build on a shared `prepareRenderInput`
helper in `@takumi-rs/helpers/renderer`, so their option types and render bodies
live in one place. Both backends check `signal` before and after resolving
fonts and images: on native, an already-aborted signal now throws before any
resource fetch.
