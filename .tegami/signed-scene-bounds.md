---
packages:
  takumi-core: minor
---

# Keep scene bounds past the canvas's top and left edges

`SceneBounds` fields are now `i32`, and a node that reaches past the canvas's top or left edge gets negative bounds instead of bounds clamped to zero.
