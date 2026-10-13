---
packages:
  "takumi-raster": patch
---

# Read layer alpha a pixel at a time

`background-clip: text` and image masks for text read a layer's alpha channel a whole pixel at a time, which makes clipped text about 6% faster.
