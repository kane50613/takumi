---
packages:
  takumi-raster:
    type: patch
---

### Stop rebuilding the sampler for every background-image pixel

Sampling a `background-image` bitmap re-derived the source dimensions, the sampling footprint and a length-checked `PixmapRef` over the whole image once per pixel, none of which depend on the pixel. That state is resolved once per tile now. A background drawn at the bitmap's own size is also handed to the compositor as the source pixmap it already is, instead of being resampled, since at 1:1 every `image-rendering` mode returns the source pixel anyway. On a 397×2160 wallpaper behind a full-height panel: 18.7 ms to 1.5 ms plain, 22.6 ms to 4.8 ms under `border-radius`, 31.3 ms to 18.4 ms under a rotation, and roughly 22% off a downscaled background. Output is unchanged.
