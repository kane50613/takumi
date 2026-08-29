---
packages:
  "@takumi-rs/core":
    type: patch
---

### Stop an oversized node from crashing the render

A node far larger than the viewport wrapped its buffer size past `u32` and crashed the render. A clip path now rasterizes only the part that reaches the canvas, an oversized shadow is dropped, and a mask too large to rasterize hides its node instead of leaving it unmasked.
