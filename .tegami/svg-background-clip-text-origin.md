---
packages:
  "takumi": patch
---

# Position `background-clip: text` layers by `background-origin` in SVG

The SVG backend positioned the layers painted through text against the border box. They now follow `background-origin`, which defaults to the padding box, as the raster and PDF backends already did.
