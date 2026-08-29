---
packages:
  "@takumi-rs/core":
    type: patch
---

### Stop an oversized node from crashing the render

A node far larger than the viewport sized its pixel buffer from a `u32` product
that wrapped. `width: 100000px` with an inset box-shadow crashed the render, and
a `clip-path` reaching hundreds of thousands of pixels asked the allocator for
hundreds of gigabytes. A clip path now rasterizes only the part that reaches the
canvas, a shadow too large to rasterize is dropped, and a mask too large to
rasterize hides its node instead of leaving it unmasked.
