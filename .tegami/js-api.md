---
packages:
  "npm:takumi-js": major
  "npm:@takumi-rs/core": major
  "npm:@takumi-rs/image-response": major
  "npm:@takumi-rs/wasm": major
---

### Make the `Renderer` constructor parameterless and register fonts with `registerFont`

The embedded default fonts decode once and stay shared across renderers.

### Remove the `createImageResponse` factory

Pass options straight to `ImageResponse`.

### Resolve lazy image loaders inside the managed `Renderer`

The render signal now comes from options.

### Accept `fonts` and `fontFamilies` in `renderAnimation` and `encodeFrames`

These match the `render` signature.

### Add a per-image `cache` mode (`auto` | `none`)

Controls decode caching one image at a time.
