---
packages:
  "cargo:takumi-raster": patch
---

### Cache text-stroke and faux-bold glyph masks

Both rasterized through `render_mask` on every draw, so CJK bold, which triggers synthesis at weight 600 and up, paid a full stroke rasterization per glyph per render. They go through the shared glyph cache now, keyed on the stroke as well as the outline. Stroked masks land on the same quarter-pixel grid as the fill, which shifts antialiasing on stroked and synthesized text by a fraction of a pixel and stops the stroke drifting from the fill it outlines.
