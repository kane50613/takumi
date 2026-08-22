---
packages:
  "@takumi-rs/core":
    type: patch
---

### Sample background-clip text at pixel centres

Text clipped to a background or mask image sampled that image at pixel corners rather than centres, blending every pixel with its neighbour. The clip is now read exactly where it should be, so a `background-clip: text` fill is half a pixel sharper, and drawing one is about a third faster.
