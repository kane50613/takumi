---
packages:
  "@takumi-rs/core": major
  "@takumi-rs/wasm": major
  takumi-js: major
---

# Stop accepting the `floyd-steinberg` dithering value

Gradient dithering now accepts only `none` and `ordered-bayer`. The deprecated `floyd-steinberg` alias is gone.
