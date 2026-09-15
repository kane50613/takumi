---
packages:
  "takumi": minor
---

# Skip paint bounds when building the paint tree

`build_stacking_contexts_unbounded` builds the same paint order without the per-node paint bounds the raster and SVG backends use to clip and cull. The paint tree uses it, so it no longer shapes every text run a second time for ink extents. The z-order, cascade, table-group and inline-box sorts switch to unstable sorts on keys that are already unique, and Tailwind utilities are bucketed instead of sorted; output is unchanged.
