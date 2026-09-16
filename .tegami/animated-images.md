---
packages:
  "takumi": minor
---

# Build without animated image support

`ImageSource::Animated` exists only when `gif`, `png` or `webp` is on. A GIF whose decoder is off lays out from its header size instead of failing to load.
