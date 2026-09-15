---
packages:
  "takumi": minor
---

# Put animated images behind an `animation` feature

`gif`, `png` and `webp` each turn on a new `animation` feature in `takumi-core` that carries the shared frame timelines and `ImageSource::Animated`. A build with none of them has no animated path, and a GIF whose decoder is off now lays out from its header size like the other formats instead of failing to load.
