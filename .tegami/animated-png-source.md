---
packages:
  "@takumi-rs/core":
    type: minor
---

### Animate APNG image sources

An animated PNG used as an image source now plays through its frames, the same way an animated GIF or WebP does. Before, only the default image was decoded and the render held that one frame for the whole animation, with nothing to signal the rest had been dropped. Subframes composite through the `fcTL` dispose and blend operations, and a default image that no `fcTL` claims stays out of the timeline.
