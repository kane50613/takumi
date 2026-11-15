---
packages:
  takumi-core: patch
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
  "@takumi-rs/helpers": patch
---

# A JSX `<svg>` sized in percentages or other units, or an `<img>` sized in percentages, draws at that size

`fromJsx` turned a `width` or `height` such as `"100%"`, `"2em"`, or `"24px"` into `NaN`, so the image drew nothing. It now leaves such a value in the element's attributes, as the same markup passed as HTML already did. The renderer reads an `<svg>`'s value as a CSS length, ignoring a negative one, and an `<img>`'s as HTML does, which takes pixels or a percentage and ignores a unit such as `em`. A percentage resolves against the containing block, as in Chromium.
