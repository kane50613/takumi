---
packages:
  "takumi": minor
---

# Build the paint tree faster

`build_scene(SceneRequest)` replaces `build_stacking_contexts`; its `paint_bounds` flag lets `paint_tree()` skip paint bounds and the text shaping they need. Z-order, cascade, table-group, and inline-box ordering use cheaper sorts.
