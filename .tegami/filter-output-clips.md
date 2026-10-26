---
packages:
  takumi-core: minor
  takumi-raster: patch
  takumi-svg: patch
  takumi-pdf: patch
---

# Correct filter and backdrop clipping

Ancestor overflow clips and an element's `clip-path` and `mask-image` now apply after filtering, preserving source pixels that contribute to blur and drop-shadow results. Rounded mask images follow the border box, and backdrop filters respect opacity, ancestor clips, and nested effect isolation. The core paint property tree exposes separate effect stages and filter clip boundaries.

In `takumi-core`, use `EffectNode::kind` and `ClipNode::kind` to inspect property nodes. Implementations of `PropertySink::pop_clip` now receive the clip being removed.
