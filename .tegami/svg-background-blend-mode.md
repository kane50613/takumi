---
packages:
  "takumi": minor
---

# Support `background-blend-mode` in SVG output

The SVG backend now blends each background layer with the layers and color beneath it, in an isolated group so nothing behind the box takes part.
