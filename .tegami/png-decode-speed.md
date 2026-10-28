---
packages:
  takumi-core: patch
---

# PNG images decode faster

PNG decoding is 13 to 85% faster natively and 13 to 81% faster in WebAssembly on most images, with the largest gains on flat graphics such as social cards, emoji, and icons, and on paletted and interlaced images. The decoded pixels are unchanged.
