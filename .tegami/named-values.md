---
packages:
  takumi-core:
    type: minor
---

### Name the values the paint helpers pass around

`BorderStyle::dash_pattern` returns a `BorderDash`. `resolve_inline_runs`, `inline_background_path`, `glyph_outlines`, and `run_decorations` are methods on `BuiltInlineLayout`, `InlineBackgroundFragment`, and `ShapedRun`.
