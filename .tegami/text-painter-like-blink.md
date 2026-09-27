---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Paint text shadows and decorations the way Chrome does

- A `text-shadow` set on a `<span>` now paints; only the block's own shadow used to.
- Text shadows now shadow the underline, overline, and line-through too, and the first listed shadow sits on top of the others.
- Each run now paints its underline and overline, then its text, then its line-through, before the next run, as CSS 2 orders them.
- PDF text shadows no longer repeat the shadowed words when the text is copied or extracted.
- PDF now applies a span's `opacity` to its text decorations.
