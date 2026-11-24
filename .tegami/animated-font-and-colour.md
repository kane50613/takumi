---
packages:
  takumi-core: patch
---

# Animations read the animated font size and colour, as in Chrome

`color`, `color-scheme`, and the font properties now apply before an element's other animations, as Chrome applies them, so animations that run together read each other's values:

- An `em` length in another animation resolves against the animated `font-size`, and `currentColor` against the animated `color`.
- An animated element with an `em` font size takes it from its parent instead of drawing it twice as large.
- A `font-size` animation interpolates an `em` endpoint against the parent's font size.
