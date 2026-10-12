---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Paint text shadows as Chrome does

- A `text-shadow` on a `<span>` paints.
- Shadows cover text decorations too, and the first listed shadow sits on top.
- Color emoji cast a shadow in their own shape.
- Each run paints underline, text, then line-through, as CSS orders them.
- Wavy, dotted and dashed underlines keep their shadows.
- PDF blurs text shadows, colors emoji shadows, and no longer repeats shadowed words when text is copied.
