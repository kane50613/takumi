---
packages:
  "takumi": minor
---

# Build without animated image support

`ImageSource::Animated` exists only with `gif`, `png`, or `webp`. A GIF keeps its header size when its decoder is off.
