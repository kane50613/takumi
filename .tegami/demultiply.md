---
packages:
  "@takumi-rs/core":
    type: patch
---

### Undo premultiplication in integers

Converting the finished canvas back to straight alpha now divides in integers rather than `f64`. Rendering any image is a few percent faster, and pixels land on the same value on every target instead of depending on the platform's float division. Semi-transparent pixels can differ by one from previous output, which is where the float landed a hair under a rounding step.
