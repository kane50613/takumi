---
packages:
  "@takumi-rs/helpers": minor
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
  takumi-paint: minor
  takumi-pdf: minor
---

# Font subsets follow what the render shapes

When a font in `fonts` declares a `unicodeRange`, as `googleFonts` subsets do, the renderer now picks the subsets to load from the characters `Renderer.needs` reads after the cascade. A subset needed only by a stylesheet's generated content, a list marker, or `text-transform` used to be skipped and now loads. `collectCodepoints` and `LIST_MARKER_CHARACTERS` are deprecated. `PdfRenderer.needs` and `Painter.needs` report the same for `takumi-pdf` and `takumi-paint`, which now pick subsets the same way, adding the characters page counters render.
