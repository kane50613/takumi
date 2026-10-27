---
packages:
  "@takumi-rs/helpers": minor
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
---

# Font subsets follow what the render shapes

When a font in `fonts` declares a `unicodeRange`, as `googleFonts` subsets do, the renderer now picks the subsets to load from the characters `Renderer.needs` reads after the cascade. A subset needed only by a stylesheet's generated content, a list marker, or `text-transform` used to be skipped and now loads. `collectCodepoints`, `subsetFonts`, and `LIST_MARKER_CHARACTERS` are deprecated.
