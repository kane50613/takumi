---
packages:
  takumi-core:
    type: minor
---

### Decide an inline outline's stroke once

`takumi_core::painter::inline_outline_stroke` says whether a `<span>`'s `outline` paints and how to dash it. The raster and SVG backends each spelled out the same dash lengths and the same list of styles an inline outline cannot draw.

Five items in `takumi_core::layout::inline` no longer leak out of the crate: `ParentFontMetrics`, `ResolvedLineMetrics`, `ResolvedInlineLineState`, `resolve_visual_inline_box`, and `text_fit_line_alignment_correction`. Nothing outside the crate used them.
