---
packages:
  takumi:
    type: minor
  "@takumi-rs/core":
    type: patch
  "@takumi-rs/wasm":
    type: patch
---

### Restore JPEG, WebP and GIF decoding

`takumi` disables `takumi-core`'s default features and had nothing to turn image decoding back on, so every build decoded PNG and ICO only. A WebP source failed with `The image format could not be determined`. The new `image-decoding` feature forwards to the core and is on by default. The napi and wasm bindings turn it on too, which is what makes those formats work from JavaScript again.
