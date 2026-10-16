---
packages:
  "takumi-paint": minor
  "takumi-core": minor
---

# Read a run's outlines one glyph at a time

`TextRun.outline()` is now `outlines()`, which returns each glyph's outline as SVG path data in the order of `glyphs`, with `null` for a glyph the run's color does not fill. A renderer can draw, animate, or hit-test each letter on its own. Replace `run.outline()` with `run.outlines().join("")`. In Rust, `PaintTree::run_outline` is now `run_outlines`.
