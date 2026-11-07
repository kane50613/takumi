---
packages:
  takumi-core: patch
---

# `calc()` takes `min()`, `max()`, and `clamp()` of numbers, and a length divided by a length

`calc(400px / clamp(1, 4 / 5, 2))` and `calc(400px / calc(400px / 400px))` used to make their declaration invalid. Inside `calc()`, `min()`, `max()`, and `clamp()` of numbers now give a number, and so does a length divided by a multiple of itself, such as `400px / 200px`. A custom property set to either works where a number does. `font-weight` also takes a math function of numbers, such as `max(500, var(--fw, 400))`, clamped to 1–1000, and `line-height` takes `min()`, `max()`, and `clamp()` as it took `calc()`.
