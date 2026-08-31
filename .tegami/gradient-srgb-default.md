---
packages:
  "takumi-core":
    type: patch
---

### Interpolate legacy-color gradients in sRGB

A gradient whose stops are all legacy colors (hex, named, `rgb()`, `hsl()`,
`hwb()`) now interpolates in sRGB unless it names a color space, matching
browsers. A stop written as `lab()`, `lch()`, `oklab()`, `oklch()`, `color()`,
or `color-mix()` keeps the Oklab default. Tailwind gradient utilities pin
`in oklab`, as Tailwind v4 does.
