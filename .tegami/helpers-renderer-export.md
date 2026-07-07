---
packages:
  npm:@takumi-rs/helpers:
    replay:
      - exit-prerelease(npm:@takumi-rs/helpers)
---

### Add the `@takumi-rs/helpers/renderer` entrypoint

The shared renderer wrapper backing the napi and wasm bindings is now exported
as `@takumi-rs/helpers/renderer`.
