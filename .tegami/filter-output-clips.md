---
packages:
  takumi-core: minor
  takumi-raster: patch
  takumi-svg: patch
  takumi-pdf: patch
---

# Correct filter and backdrop clipping

Ancestor overflow clips and an element's `clip-path` and `mask-image` now apply after filtering, preserving source pixels for blur and drop shadows. Rounded mask images follow the border box, and raster backdrop filters respect opacity, ancestor clips, and nested effect isolation. In `takumi-core`, the paint property tree exposes separate effect stages and filter clip boundaries through `EffectNode::kind` and `ClipNode::kind`, and `PropertySink::pop_clip` receives the clip being removed.
