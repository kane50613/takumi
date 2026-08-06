---
packages:
  takumi-pdf:
    type: minor
---

### Fade elements with `mask-image`

Gradient mask layers now apply, as a PDF soft mask holding the mask's own vector content. The masked element and its descendants stay vector: nothing is rasterized to fade an element out.

`url()` mask sources are still ignored, and the mask is an alpha mask, which is what `mask-mode: match-source` resolves to for an image source.
