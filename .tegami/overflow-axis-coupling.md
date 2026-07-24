---
packages:
  "cargo:takumi-core": patch
  "cargo:takumi-raster": patch
---

### Couple the overflow axes and stop mixed overflow from blanking a node

`overflow-x` and `overflow-y` were read straight off the computed style, so `overflow-x: hidden` next to `overflow-y: visible` stayed mixed. CSS Overflow 3 says a `visible` axis paired with one that is neither `visible` nor `clip` computes to a scrolling value instead, which is why Chrome clips both axes there. `resolve_overflows` now applies that coupling, so the pair reaches layout, painting, and the SVG backend already resolved. `clip` next to `visible` is a legal combination and still passes through untouched.

That legal pair then hit a second bug. The mask builder marks an unclipped axis with `u32::MAX`, and the identity-transform fast path narrowed it with `as i32`, which truncates to `-1` rather than saturating. The clip rectangle came out empty, so the node rendered nothing at all. The comparison now clamps before narrowing, matching the rotated path, which had always compared in `u32`.
