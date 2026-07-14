---
packages:
  "cargo:takumi-raster": patch
---

### Render animation frames in parallel chunks

`write_animation` renders one chunk of rayon threads' worth of frames in parallel while streaming into the encoder, keeping at most one chunk of raw frames in memory.
