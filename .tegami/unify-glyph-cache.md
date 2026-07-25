---
packages:
  "cargo:takumi-core": minor
  "cargo:takumi-raster": patch
  "cargo:takumi-svg": patch
---

### Replace `ResolvedGlyphPlacement` with `geometry::Placement`

`Placement` moves into `takumi_core::geometry` and takes over from `ResolvedGlyphPlacement`, which described the same four fields. `BuiltInlineLayout::resolved_glyphs` is now keyed to `Arc<ResolvedGlyph>`, so a glyph cache hit stops copying the outline commands.
