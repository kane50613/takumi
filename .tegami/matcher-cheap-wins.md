---
packages:
  "cargo:takumi-core": patch
---

### Cut wasted work in style matching

`record_matches` no longer pushes entries for empty declaration blocks, and the per-node ancestor bloom filters (one multi-kilobyte copy per node) are replaced by a single counting filter walked along the DFS ancestor chain. Rendered output is unchanged.
