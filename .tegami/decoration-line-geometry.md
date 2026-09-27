---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Draw text decorations where Chrome draws them

Underlines, overlines and line-throughs now follow Chrome's geometry more closely:

- A line spans the text exactly, with antialiased ends, instead of widening to whole pixels.
- `text-decoration-skip-ink` cuts on whole device pixels, also cuts overlines, and looks for glyphs across the whole band a wavy or double line paints.
- It no longer cuts around CJK characters, Hangul, emoji, `/`, `\` or `_`.
- A double or wavy line keeps its offset from the unrounded thickness, and a wavy line keeps its stroke width.
