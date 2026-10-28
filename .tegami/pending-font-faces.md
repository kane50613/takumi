---
packages:
  takumi-core: minor
  takumi-pdf: patch
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
  "@takumi-rs/helpers": patch
  takumi-js: patch
  takumi-paint: patch
---

# Font subsets load as a browser loads them

When `fonts` holds `unicodeRange` subsets, as `googleFonts` returns, a render now loads only the subsets that font matching reaches for each character, through that text's own font stack, weight, and style. With `font-family: Inter, "Noto Sans JP"`, Latin text no longer loads Noto Sans JP's Latin subset, and a bold subset loads only when bold text uses it.
