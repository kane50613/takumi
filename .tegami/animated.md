---
packages:
  "takumi": patch
---

# Encode GIF and decode WebP animations faster

- Native builds pick GIF palettes in parallel. A 24-frame 1200x630 GIF encodes 2.5x faster, byte for byte the same.
- Native builds decode animated WebP with libwebp, about 2.5x faster. The Node.js binary is 180KB smaller.
