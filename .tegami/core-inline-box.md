---
packages:
  takumi-core:
    type: minor
  takumi-pdf:
    type: patch
---

### Resolve inline boxes once, for every backend

`takumi_core::layout::inline_box::resolve_inline_box` decides what an inline box holds and lays out an inline-level container at the size its line gave it. The raster, SVG, and PDF backends had a copy each, and the PDF one was missing.

A replaced inline box, such as an `<img>` between two words, now paints its background, border, shadow, and outline in PDF output.
