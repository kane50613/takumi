---
packages:
  "cargo:takumi-raster": patch
  "@takumi-rs/core": patch
---

### Blend animated WebP frames by default on the native binding

`AnimatedWebpOptions::builder()` left `blend` and `dispose` unset, so they fell back to `false` while the type's `Default` and the wasm backend use `blend: true`. Native animated WebP now alpha-blends frames over prior content like wasm does, so animations with partially transparent later frames render the same on both backends. The builder defaults are pinned to the `Default` values.
