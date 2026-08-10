---
packages:
  takumi:
    type: minor
  takumi-core:
    type: minor
  "@takumi-rs/core":
    type: patch
  "@takumi-rs/wasm":
    type: patch
---

### Restore JPEG, WebP and GIF decoding

`takumi` disables `takumi-core`'s default features and never re-enables image decoding, so every build decoded PNG and ICO only. A WebP source failed with `The image format could not be determined`. `image-decoding` is now a `takumi` feature as well, on by default, and it splits into `jpeg`, `webp` and `gif` for a build that wants one format and not the others. The napi and wasm bindings turn it on too.
