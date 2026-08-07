---
packages:
  takumi-core:
    type: patch
---

### Animate colours in sRGB

An animated or transitioned colour now interpolates in sRGB with premultiplied alpha, the space CSS Color 4 gives legacy colours, instead of Oklab. Midpoints between distant hues shift to match a browser.
