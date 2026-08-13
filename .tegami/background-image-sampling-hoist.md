---
packages:
  takumi-raster:
    type: patch
---

### Stop rebuilding the sampler for every background-image pixel

Sampling a bitmap background rebuilt the source dimensions, the sampling footprint and a length-checked `PixmapRef` for every pixel. That state is resolved once per tile now. A background drawn at the bitmap's own size is composited as the source pixmap instead of being resampled. Output is unchanged.
