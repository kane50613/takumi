---
packages:
  takumi-core: patch
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
  "@takumi-rs/helpers": patch
---

# A JSX `<svg>` or `<img>` sized in percentages or other units draws at that size

`fromJsx` turned a `width` or `height` such as `"100%"`, `"2em"`, or `"24px"` into `NaN`, so the image drew nothing. It now leaves such a value in the element's attributes, and the renderer reads an `<svg>`'s as a CSS length and an `<img>`'s as HTML does, the way it already read the same markup passed as HTML. A percentage resolves against the containing block, as in Chromium.
