---
packages:
  "takumi-pdf": patch
---

# Fix PDF rendering bugs

- Gradients in repeated backgrounds no longer draw 4/3 too large or off-center.
- Text under `opacity`, a blend mode or a mask keeps the right edge of its last glyph.
- A tagged PDF no longer panics on an inline `<img>` with a background, border or shadow.
- A text shadow past a page break stays with its line instead of vanishing.
