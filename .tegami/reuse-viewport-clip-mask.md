---
packages:
  "takumi": patch
---

# Use less memory for a clip that covers the image

A clip or `clip-path` that covers the whole image, such as `overflow: hidden` on the root, no longer holds a second copy of its mask while rendering.
