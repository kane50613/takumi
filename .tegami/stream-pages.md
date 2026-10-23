---
packages:
  takumi-raster: patch
---

# Hand each page to `render_pages` before the whole document is laid out

`render_pages` calls back with a page as soon as no page still to lay out can paint onto it, so a long document holds one page's scene at a time. Layout errors, including `TooManyPages`, can now arrive after the callback has received earlier pages.
