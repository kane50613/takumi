---
packages:
  "takumi": minor
---

# Build without the PNG decoder

`png` joins `jpeg`, `webp` and `gif` under the default-on `image-decoding` feature. With it off, a PNG or APNG source lays out from its header size and cannot be drawn, PNG bitmap glyphs are skipped, and `ImageBuffer::encode_png` is gone.
