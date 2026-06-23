---
packages:
  "cargo:takumi-css": patch
  "cargo:takumi-raster": patch
---

### Apply `fontFamilies` as the default font

Text without an explicit `font-family` ignored the render's `fontFamilies` (and
any fonts passed through it), always using the embedded fallback. The
`fontFamilies` stack is now the root default, so passed fonts render without a
per-node `font-family`.
