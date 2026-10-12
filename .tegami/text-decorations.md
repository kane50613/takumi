---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Draw text decorations as Chrome does

- Underlines, overlines and line-throughs sit and size as Chrome draws them, on whole pixels.
- `text-underline-position`, `text-underline-offset` and `text-decoration-thickness: from-font` follow Chrome.
- `text-decoration: overline 12px red` keeps its thickness.
- A decoration reaches the text of every in-flow box inside its element, and nested decorations all draw.
- An underline uses the baseline and font of the element that sets it.
- `text-decoration-skip-ink` also cuts overlines and wavy lines. It no longer cuts around CJK, Hangul, emoji, `/`, `\` or `_`.
- Text under `opacity`, `filter` or a blend mode keeps decorations that reach past its box.
- PDF applies a span's `opacity` to its decorations.
