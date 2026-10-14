---
packages:
  "takumi-raster": patch
---

# Keep clip masks to the pixels they cover

A clip such as `overflow: hidden` with rounded corners keeps a mask only over the pixels it covers instead of one the size of the whole image. Nested clips render about 13% faster with about 28% less peak memory, and images inside rounded clips render 5–7% faster.
