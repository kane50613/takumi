---
packages:
  takumi-core: minor
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
  "@takumi-rs/helpers": minor
  takumi-js: patch
  takumi-paint: patch
  takumi-pdf: patch
---

# Font subsets cover the text inside SVG images

When `fonts` holds `unicodeRange` subsets, a render now also loads the ones that the `text` elements of its SVG images need, such as chart labels. This covers an inline SVG, an SVG `data:` URI, and an SVG passed in or fetched through `images`. `subsetFonts` is deprecated, since its remaining use was text inside SVG images.
