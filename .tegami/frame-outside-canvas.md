---
packages:
  takumi-core:
    type: patch
---

### Clip animation frames that start outside the canvas

A GIF or WebP frame whose origin lies beyond the logical screen no longer panics the decoder. The frame is clipped away instead, as browsers do.
