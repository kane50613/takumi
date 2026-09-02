---
packages:
  takumi-core:
    type: minor
---

### Read render-wide settings through RenderContext methods

`time_ms`, `draw_debug_border`, and `dither_gradients` are methods on `RenderContext` now, since the values are shared by every context of one render. `RenderContext::builder()` is unchanged.
