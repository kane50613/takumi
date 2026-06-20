---
packages:
  "cargo:takumi": major
  "cargo:takumi-raster": minor
---

### Model image output quality per format

`ImageOutputFormat::Jpeg`/`WebP` carry a `Quality`, a new `WebPLossless` variant replaces lossless WebP (a `lossless` flag in the napi/wasm bindings), and `write_image` drops its quality argument.
