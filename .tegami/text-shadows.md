---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Paint text shadows as Chrome does

- A `text-shadow` set on a `<span>` paints. Only the block's own shadow used to.
- Text shadows also shadow underlines, overlines and line-throughs, and the first listed shadow sits on top.
- Each run paints its underline and overline, then its text, then its line-through, before the next run, as CSS 2 orders them.
- PDF text shadows no longer repeat the shadowed words when the text is copied or extracted.
