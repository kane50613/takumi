---
packages:
  takumi-raster: patch
  takumi-pdf: patch
---

# Free the layout data of a page once it is drawn

`render_pages` and `render` free the layout data a page held, such as its text lines and cached layouts, once the page is drawn and no later page paints from it. The source tree and the tree of boxes stay in memory for the whole render, so peak memory still grows with the size of the document. A document that lays out to its end before the first page is drawn frees nothing early.
