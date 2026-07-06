---
packages:
  cargo:takumi-core:
    type: minor
  cargo:takumi:
    type: major
---

### Seal `parley::Layout` out of the inline-layout boundary

`BuiltInlineLayout::{layout, custom_inline_boxes}` are now private; the
measure-only walk moves into `BuiltInlineLayout::measure_runs`, returning
core-owned `MeasuredInlineRun`/`MeasuredInlineBox` (run text borrows the
layout). `get_parent_font_metrics`, `resolve_inline_line_metrics`,
`resolve_inline_line_states`, and `scale_text_fit_x` are no longer public.
