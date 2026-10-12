---
packages:
  "takumi-core": patch
  "takumi": patch
---

# Rasterize inline SVG images once

An `<img>` whose `src` is SVG markup now goes through the render's image cache, as `data:` URIs already do. Repeated renders reuse its parsed tree and rasters instead of redrawing them. Real templates render about 3% faster.
