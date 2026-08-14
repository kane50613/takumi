---
packages:
  "@takumi-rs/core":
    type: patch
  takumi-pdf:
    type: patch
---

### Close the gap between two boxes on a fractional parent

A box's position snapped to the pixel grid against its parent while its size snapped against the page. A parent sitting on a fraction, such as a container padded in points, pushed the two apart and left a hairline of background between boxes that should meet.
