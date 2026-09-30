---
packages:
  "takumi": patch
---

# Composite `drop-shadow()` in premultiplied colour

- A coloured `drop-shadow()` fades out as Chrome's does. It used to paint an opaque halo of its colour, and an element's anti-aliased edge came out lighter than the shadow behind it.
