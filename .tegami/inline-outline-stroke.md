---
packages:
  takumi-core:
    type: minor
---

### Decide an inline outline's stroke once

`SizedFontStyle::outline_stroke` says whether a `<span>`'s `outline` paints and how to dash it. The raster and SVG backends each spelled out the same dash lengths and the same list of styles an inline outline cannot draw.

Nine items in `takumi_core` no longer leak out of the crate, none of which had a caller outside it: `ParentFontMetrics`, `ResolvedLineMetrics`, `ResolvedInlineLineState`, `resolve_visual_inline_box`, `text_fit_line_alignment_correction`, `BorderPaint`, `border_paint`, `outline_paint`, and `outline_geometry`.
