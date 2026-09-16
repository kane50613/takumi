---
packages:
  "takumi": minor
---

# Build the paint tree faster

`paint_tree()` skips paint bounds and the text shaping they need. Z-order, cascade, table-group, and inline-box ordering use cheaper sorts.
