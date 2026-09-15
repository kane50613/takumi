---
packages:
  "takumi": minor
---

# Put the PNG decoder behind a `png` feature

`png` joins `jpeg`, `webp` and `gif` under the default-on `image-decoding` feature. With it off, a PNG source still lays out from its header size, APNG plays as a still, bitmap glyphs stored as PNG are skipped, and `ImageBuffer::encode_png` is gone. `takumi-paint` builds without it.
