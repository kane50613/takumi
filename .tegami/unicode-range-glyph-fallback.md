---
packages:
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
  takumi-pdf: patch
  takumi-paint: patch
---

# A `unicodeRange` subset without a glyph falls back to the next family again

A font in `fonts` whose `unicodeRange` claims a character it has no glyph for now hands that character on to the next family in the stack, as a browser does and as 0.18 did. Before, the render left the next family's subset unloaded. The PDF render then failed with "No registered font covers". That error now reads "No font the text can use draws", and suggests checking each font's `unicodeRange` and the `font-family` stack, because a registered font can have the glyph and still be out of reach.
