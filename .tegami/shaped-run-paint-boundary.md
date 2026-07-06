---
packages:
  cargo:takumi-core:
    type: minor
---

### Replace `parley::GlyphRun` with a core-owned `ShapedRun` at the paint boundary

`PositionedInlineRun::glyph_run` is now `ShapedRun` (owned glyphs, brush, metrics,
font data) instead of `parley::GlyphRun<'l, InlineBrush>`; `PositionedInlineRun`
and `InlineRunLayout` drop their lifetime. `run_decorations` takes `&ShapedRun`.
