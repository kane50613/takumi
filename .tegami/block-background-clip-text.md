---
packages:
  "takumi": patch
  "takumi-pdf": patch
  "takumi-paint": minor
---

# Show a box's `background-clip: text` background through the text of every box inside it

A box with `background-clip: text` now shows its background through the text of its child blocks, inline-blocks, floats and positioned children, not just its own inline text, as in Chrome. It paints with the rest of the background, so `text-shadow` lands on top of it and a child's own background covers it.

In the paint tree, that background is now a `masked` drawable among the box's own drawables, its mask the text's outlines as `fill` drawables, instead of a `masked` drawable on the text node with a `glyphs` mask.
