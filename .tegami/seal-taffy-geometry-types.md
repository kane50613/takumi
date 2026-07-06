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
instead of `&taffy::Layout`. `taffy` remains the layout engine at the
`compute_layout` input seam (`AvailableSpace`, `NodeId`, `Style` are unaffected).
