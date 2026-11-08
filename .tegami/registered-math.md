---
packages:
  takumi-core: patch
---

# A registered custom property takes the math functions

A custom property that `@property` registers now takes the math functions in its value, as Chromium does. `--angle: calc(1turn / 4)` computes to `90deg`, `--size: min(10px, 2em)` to px, and a `<length-percentage>` that needs the percentage basis keeps its function with the other units computed, so `clamp(1em, 5%, 30px)` at a 16px font is `clamp(16px, 5%, 30px)`.
