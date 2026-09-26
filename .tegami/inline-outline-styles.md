---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Draw every `outline-style` on inline elements

An inline element's `outline` now draws `double`, `groove`, `ridge`, `inset`, and `outset`, which used to paint nothing. Dashed and dotted outlines space their dashes the way Chrome does, starting each edge on a dash, and a translucent dashed outline no longer darkens where its dashes overlap at the corners.
