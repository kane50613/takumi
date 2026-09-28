---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Paint gradients and images on inline spans

A `background-image` on an inline `<span>` now paints across the lines it wraps over, laid out as one continuous strip like Chrome's. A span with `background-clip: text` shows its gradient through its glyphs, so gradient text inside a heading no longer disappears.
