---
"takumi": major
"takumi-raster": minor
---

Model image output quality per format: add `Quality` on the `ImageOutputFormat::Jpeg`/`WebP` variants, split lossless WebP into an `ImageOutputFormat::WebPLossless` variant (with a `lossless` flag in the napi/wasm bindings), and drop the separate `write_image` quality argument
