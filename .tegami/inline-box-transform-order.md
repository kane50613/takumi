---
packages:
  takumi-raster:
    type: patch
---

### Place inline content inside its container's transform

A rotated or scaled container drew its inline boxes, its inline images and its outline in the wrong place. Those offsets were applied in device space, after the transform, instead of in the container's own coordinates. A container with a plain translation was never affected.
