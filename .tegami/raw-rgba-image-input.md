---
packages:
  "cargo:takumi-core": minor
  "cargo:takumi": minor
  "npm:@takumi-rs/helpers": minor
  "npm:@takumi-rs/core": minor
  "npm:@takumi-rs/wasm": minor
  "npm:takumi-js": minor
---

### Accept raw RGBA pixels as an image source

An image node's `src` takes `{ width, height, data, premultiplied? }` and renders the pixels without decoding; the `Bitmap` JSX helper wraps it as an `<img>`.
