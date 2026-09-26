---
packages:
  "takumi": patch
---

# Stop `background-clip: border-area` from painting the border color twice

With `background-clip: border-area`, the raster backend filled the border ring with the background over the border's own color, and the border then painted on top again. A translucent border came out darker than in a browser. The ring now takes only the background, as it does in the SVG backend.
