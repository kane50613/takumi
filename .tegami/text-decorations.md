---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Draw text decorations as Chrome does

- Underlines, overlines and line-throughs round their top edge to the nearest pixel and their thickness down to a whole pixel, at least 1px.
- PDF applies a span's `opacity` to its text decorations.
