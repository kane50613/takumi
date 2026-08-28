---
packages:
  "@takumi-rs/core":
    type: patch
---

### Stop a box narrower than its padding from panicking

A box whose horizontal padding exceeds its own width left the text a negative
width to lay out in, which tripped an assertion inside the text layouter and
crashed the render. The width the text lays out against now stops at zero.
