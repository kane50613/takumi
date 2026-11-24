---
packages:
  takumi-core: patch
  takumi-pdf: patch
  takumi-raster: patch
---

# Stop a box at the top of a page from painting a line on the page before

A box that starts a page no longer paints a thin line along the bottom of the page before it when the page area is not a whole number of pixels, such as an A4 page of 793.7 × 1122.52 px.

The page area now rounds up to whole pixels, as in Chromium:

- Content lays out on the rounded area, and PDF pages clip to it. The PDF page size does not change.
- Page images take the rounded size, so they can be a pixel larger.
- Viewport units take the rounded area too, so `vh` can be a pixel taller than in Chromium.
