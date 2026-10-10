---
packages:
  "takumi": patch
---

# Decode animated WebP faster

Native builds decode animated WebP with libwebp, which they already ship for encoding, so frames decode about 2.5x faster and the Node.js binary is 180KB smaller. Decoded frames are unchanged.
