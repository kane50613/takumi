---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Paint SVG and PDF output like the image output

- A block's loose text no longer paints the block's background a second time, so a negative `z-index` child behind it stays visible.
- Spans that set their own `font-size` inside a `font-size: 0` block paint.
- PDF paints text that overflows a box with no width or height, and no longer writes `NaN` for a zero-sized run.
- PDF paints the content that overflows an `inline-block` with `height: 0`.
- SVG path data keeps the letter of a moveto that follows another, so the second move no longer becomes a stray line.
