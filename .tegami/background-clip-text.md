---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Show `background-clip: text` through the text Chrome shows it through

- The background shows through the text of child blocks, inline-blocks, floats and positioned children, not just the box's own text.
- Nested `background-clip: text` spans each show their background, outer span first.
- The background shows through underlines, overlines and line-throughs too.
- It shows through text at `opacity: 0` or `scale(0)`, and skips the floats and positioned boxes Chrome skips.
- `text-shadow` paints on top of the clipped background, and a child's own background covers it.
- A clipped background renders about 30% faster and peaks 35% lower in memory.
