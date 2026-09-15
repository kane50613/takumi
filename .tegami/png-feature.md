---
packages:
  "takumi": minor
---

# Put the PNG decoder behind a `png` feature

`png` joins `jpeg`, `webp` and `gif` under the default-on `image-decoding` feature. With it off, a PNG or APNG source lays out from its header size and cannot be drawn, bitmap glyphs stored as PNG are skipped, and `ImageBuffer::encode_png` is gone. `takumi-paint` builds without it.
