---
packages:
  "takumi": minor
---

# Compile the animated image path only with an animated decoder

`ImageSource::Animated` and the frame timelines behind it now exist only when at least one of `gif`, `png` or `webp` is on, so a build without them carries no animation code. A GIF whose decoder is off lays out from its header size like the other formats instead of failing to load.
