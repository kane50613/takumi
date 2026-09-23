---
packages:
  "takumi": patch
---

# Build the raster backend without the `svg` feature

`takumi-raster` with `default-features = false` failed to compile because image drawing was gated behind `svg`. It now builds, and only SVG sources need the feature.
