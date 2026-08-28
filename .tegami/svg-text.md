---
packages:
  "@takumi-rs/core":
    type: minor
  "@takumi-rs/wasm":
    type: minor
  "takumi-js":
    type: minor
  "takumi-pdf":
    type: minor
---

### Render `<text>` elements in SVG image sources

SVG images with `<text>`, `<tspan>` and `textPath` now draw their text using
the registered fonts instead of dropping it. Glyphs render from font outlines;
color emoji glyphs inside SVG text are not supported.
