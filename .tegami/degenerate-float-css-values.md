---
packages:
  "@takumi-rs/core":
    type: patch
---

### Keep degenerate CSS values out of layout

`aspect-ratio` with a zero, negative, or non-finite ratio (such as `1/0`) now behaves as `auto`, and an infinite percentage length is clamped instead of feeding infinity into layout.
