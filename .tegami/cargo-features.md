---
packages:
  "takumi": minor
---

# Build `takumi` with fewer Cargo features

- `ImageSource::Animated` exists only with `gif`, `png`, or `webp`. A GIF keeps its header size when its decoder is off.
- The `png` feature is on by default through `image-decoding`. Without it, PNG and APNG sources keep their header size but cannot be drawn, PNG bitmap glyphs are skipped, and `ImageBuffer::encode_png` is gone.
- `svg-sizing` reads an SVG's `width`, `height`, and `viewBox` without the renderer. The `svg` feature includes it.
- The raster backend builds without the `svg` feature. Only SVG sources need it.
