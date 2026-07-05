---
packages:
  "cargo:takumi-raster": minor
  "cargo:takumi": minor
  "npm:@takumi-rs/core": patch
  "npm:@takumi-rs/wasm": patch
---

### Cap animation frame rate per format

Browsers clamp any animation frame of 10ms or less to 100ms, so a high frame
rate stalls instead of playing fast. `write_animation` now rejects a frame rate
above `AnimationFormat::max_fps` with the new `AnimationFrameRateTooHigh` error:
90 fps for WebP and APNG, 50 fps for GIF (centisecond delays). The napi and WASM
`renderAnimation` bindings surface the error.
