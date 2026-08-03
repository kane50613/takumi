---
packages:
  "cargo:takumi-core": minor
---

### Add break-before, break-after, and break-inside

The three fragmentation properties parse with their page values: `break-before: page`, `break-after: page`, and `break-inside: avoid`. Raster and SVG output ignores them; the paged PDF backend consumes them. `ShapedRun` now carries its source text range and per-glyph cluster ranges for text-extraction backends, and the JPEG, WebP, and GIF decoders sit behind a new default-on `image-decoding` feature so slim builds can drop them.
