---
packages:
  "@takumi-rs/helpers": minor
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
  takumi-js: patch
  takumi-paint: minor
  takumi-pdf: minor
---

# Font subsets load as a browser loads them

When `fonts` holds `unicodeRange` subsets, as `googleFonts` returns, a render now loads only the subsets that font matching reaches for each character, through that text's own font stack, weight, and style. With `font-family: Inter, "Noto Sans JP"`, Latin text no longer loads Noto Sans JP's Latin subset, and a bold subset loads only when bold text uses it.

The text comes from the styled tree, so a subset needed only by a stylesheet's generated content, a list marker, `text-transform`, the `text` elements of an SVG image, or a `takumi-pdf` page counter now loads. `collectCodepoints`, `LIST_MARKER_CHARACTERS`, and `subsetFonts` are deprecated.
