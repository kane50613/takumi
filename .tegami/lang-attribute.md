---
packages:
  "cargo:takumi": minor
  "cargo:takumi-core": minor
  "cargo:takumi-css": minor
  "cargo:takumi-raster": minor
  "cargo:takumi-svg": minor
  "npm:takumi-js": minor
  "npm:@takumi-rs/core": minor
  "npm:@takumi-rs/helpers": minor
  "npm:@takumi-rs/wasm": minor
  "npm:@takumi-rs/image-response": minor
---

### Render text language-aware with the `lang` attribute

The `lang` attribute sets a node's language and inherits to its descendants; a
nested `lang` overrides, and an empty or invalid value clears it. The language
reaches the shaper as the locale, so the same code point draws its
language-specific glyph (Han unification across `ja` / `zh` / `ko`) and lines
break per language. Pass `lang` in the render options to set a default for the
whole tree.
