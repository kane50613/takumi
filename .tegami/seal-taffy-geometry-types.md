---
packages:
  cargo:takumi-core:
    type: minor
---

### Seal `taffy` geometry types out of the public API

`layout::border::BorderProperties`, `shadow::SizedShadow`, `layout::inline::InlineBoxItem`,
and the other geometry-touching public items now use core-owned
`geometry::{Size, Rect, Point}` instead of `taffy::{Size, Rect, Point}`.
`layout::tree::LayoutResults::layout` returns an owned `geometry::ComputedLayout`
instead of `&taffy::Layout`. `LayoutTree::compute_layout` and the paint scene
(`build_stacking_contexts`, `NodePaint`, `layout::tree::OrderedChild`) now use
core-owned `geometry::{AvailableSpace, NodeId}` instead of `taffy::{AvailableSpace,
NodeId}`; `NodeId::ROOT` replaces the removed `root_node_id()` accessors. `taffy`
remains the layout engine at the `compute_layout` internals and `Style`
construction.
