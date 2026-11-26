---
packages:
  takumi-core: patch
---

# Animations read the animated font size and colour

Animations that run together now read each other's `font-size`, `color`, and `color-scheme`, as in Chrome, and an animated element takes an `em` font size from its parent.
