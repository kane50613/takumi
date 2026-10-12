---
packages:
  "takumi": minor
---

# Read painted values from a node tree

- With takumi-core's `paint-tree` feature, `takumi_core::paint_tree::paint_tree` returns decorations, image placement and shaped text in paint order, in device pixels.
- `SceneRequest::build` replaces `build_stacking_contexts`. Set `paint_bounds: false` to skip paint bounds and the text shaping they need.
- `Fonts::face_family` names a shaped run's registered family.
