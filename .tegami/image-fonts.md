---
packages:
  takumi-core: minor
  takumi-pdf: minor
  "@takumi-rs/core": minor
  "@takumi-rs/wasm": minor
  "@takumi-rs/helpers": minor
  takumi-js: minor
  takumi-paint: minor
---

# Emoji draw as glyphs of an image font

`takumi-js` draws emoji from the CDN set `emoji` names as glyphs, so they keep the line's baseline, spacing, and decorations, and copying them from a PDF gives back the emoji. An emoji the set has no SVG for draws from the next font instead of failing the render. Every renderer takes `imageFonts`, fonts whose glyphs are images keyed by grapheme cluster, and its `prepare` loads the ones a source draws through each font's `load`. `emojiImageFont` from `@takumi-rs/helpers/emoji` builds the font for a CDN set. In Rust, `Fonts::register_image_face` registers one and `RenderNeeds::glyphs` reports the clusters it lacks.
