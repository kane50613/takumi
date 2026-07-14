---
packages:
  "cargo:takumi-core": minor
  "cargo:takumi-raster": patch
---

### Memoize GIF frames at draw size

Frames past the first decode scaled to the box the GIF is drawn into, so an animation's memoized timeline holds draw-sized frames instead of canvas-sized ones. Adds `GifSource::frame_at_time_covering`.
