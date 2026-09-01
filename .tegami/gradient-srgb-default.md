---
packages:
  "takumi-core":
    type: patch
---

### Interpolate legacy-color gradients in sRGB

A gradient with only legacy color stops (hex, named, non-relative `rgb()`,
`hsl()`, `hwb()`) interpolates in sRGB, as browsers do. Modern stops keep
Oklab, and Tailwind gradient utilities pin `in oklab`.
