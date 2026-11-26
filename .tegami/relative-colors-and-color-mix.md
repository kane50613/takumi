---
packages:
  takumi-core: patch
---

# Relative colours keep their channels, and `color-mix()` defaults to Oklab

- `hsl(from white 120 50 50)` keeps the hue it is written with and resolves to the same green as in Chrome, where a powerless origin hue had dropped it to zero.
- `color-mix(red, blue)` mixes in Oklab as in Chrome, and a mix in Oklab serializes without the `in oklab,` it implies.
