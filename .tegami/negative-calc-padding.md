---
packages:
  takumi-core: patch
---

# A negative `calc()` padding clamps to zero

A `padding` written as a `calc()` that comes out negative, such as `calc(0px - 4px)`, was applied as a negative padding. It is now clamped to zero, as browsers do.
