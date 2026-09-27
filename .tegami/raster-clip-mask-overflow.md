---
packages:
  "takumi": patch
---

# Apply `clip-path`, `mask-image`, and `overflow` together in images

An element with more than one of `clip-path`, `mask-image`, and a clipping `overflow` now applies all of them in the image output. It used to apply only the first, so a masked or clip-pathed box with `overflow: hidden` let its children spill past the box.
