---
packages:
  "takumi": minor
---

# Record what a node tree paints as a tree of used values

`takumi_core::paint_tree::paint_tree` lays out a node tree and returns each box's used background, border, shadows, outline, image placement, and shaped text runs in paint order, with device-pixel geometry and absolute transforms. `Fonts::face_family` names the registered family of a shaped run's face.
