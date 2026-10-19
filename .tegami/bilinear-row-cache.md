---
packages:
  takumi: patch
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
---

# Scale images up faster

A magnified image lerped every source row across the destination width again for each destination row that read it. Each source row is now lerped once and reused, so drawing a small image across a large box takes about 40% less time. The output is unchanged.
