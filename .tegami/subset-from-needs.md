---
packages:
  "@takumi-rs/helpers": minor
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
  takumi-paint: minor
  takumi-pdf: minor
---

# Font subsets follow what the render shapes

When a font in `fonts` declares a `unicodeRange`, as `googleFonts` subsets do, the renderer now picks the subsets to load from the characters it reads after the cascade. A subset needed only by a stylesheet's generated content, a list marker, or `text-transform` used to be skipped and now loads. `takumi-pdf` also counts the characters page counters render. `collectCodepoints` and `LIST_MARKER_CHARACTERS` are deprecated.
