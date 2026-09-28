---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Draw outlines as Chrome does

- An inline element's `outline` draws `double`, `groove`, `ridge`, `inset` and `outset`, which used to paint nothing.
- Dashed and dotted outlines start each edge on a dash, and a translucent dashed outline no longer darkens where its dashes overlap at the corners.
- PDF draws the `outline` of an inline element as one contour across line breaks, at the element's opacity.
