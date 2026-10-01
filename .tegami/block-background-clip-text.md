---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Show a box's `background-clip: text` background through the text of every box inside it

A box with `background-clip: text` now shows its background through the text of its child blocks, inline-blocks, floats and positioned children, not just its own inline text, as in Chrome. It paints with the rest of the background, so `text-shadow` lands on top of it and a child's own background covers it.
