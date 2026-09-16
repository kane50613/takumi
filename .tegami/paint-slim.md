---
packages:
  "takumi": minor
---

# Build the paint tree faster

`paint_tree()` skips the per-node paint bounds and the extra text shaping they need. The z-order, cascade, table-group and inline-box sorts do less work; output is unchanged.
