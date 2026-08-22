---
packages:
  "@takumi-rs/core":
    type: patch
---

### Draw whole-pixel images without resampling them

An image placed at a whole-pixel offset is copied row by row instead of going through the sampling pipeline. Drawing one into a node is about three times faster; clips, shadows and blurs that composite an image get a few percent.
