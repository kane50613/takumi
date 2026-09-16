---
packages:
  "takumi": minor
---

# Build without PNG decoding

The `png` feature is on by default through `image-decoding`. Without it, PNG and APNG sources keep their header size but cannot be drawn, PNG bitmap glyphs are skipped, and `ImageBuffer::encode_png` is gone.
