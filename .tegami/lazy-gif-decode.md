---
packages:
  "cargo:takumi-core": minor
  "cargo:takumi-raster": patch
---

### Decode GIF frames lazily

Decode only the first GIF frame up front and the rest on first sample past it; static renders no longer decode the whole animation. `GifSource::frame_at_time` returns `Arc<ImageBuffer>` and `RenderedImage::Borrowed` becomes `Sampled`.
