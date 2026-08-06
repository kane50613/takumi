---
packages:
  takumi-pdf:
    type: minor
---

### Embed SVG image sources as vectors

SVG images previously rasterized at 2× their placed size, leaving small logos soft next to vector text. They now embed as real paths, gradients and clips, sharp at any zoom. Filters and bitmaps embedded inside an SVG still rasterize at 2×.
