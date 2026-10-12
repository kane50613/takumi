---
packages:
  "takumi-core": minor
  "takumi": patch
---

# Composite pixels faster

Blending, masking and PNG scanline work run in 16-bit vector lanes, with the same pixels as before. Real templates render about 3% faster.
