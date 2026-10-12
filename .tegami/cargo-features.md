---
packages:
  "takumi": minor
---

# Build `takumi` with fewer Cargo features

- `ImageSource::Animated` needs `gif`, `png`, or `webp`. A GIF still reports its size without its decoder.
- `png` is on by default through `image-decoding`. Without it, PNG sources report their size but cannot draw, PNG bitmap glyphs are skipped, and `ImageBuffer::encode_png` is gone.
- `svg-sizing` reads an SVG's `width`, `height`, and `viewBox` without the renderer. `svg` includes it.
- The raster backend builds without `svg`. Only SVG sources need it.
