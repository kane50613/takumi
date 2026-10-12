---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Paint SVG and PDF output like the image output

- A block's background no longer paints twice, so a negative `z-index` child behind it shows.
- Spans with their own `font-size` inside a `font-size: 0` block paint.
- PDF paints text that overflows a zero-sized box, and content that overflows a `height: 0` inline-block.
- SVG path data no longer turns a second moveto into a stray line.
