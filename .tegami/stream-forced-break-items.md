---
packages:
  takumi-core: patch
  takumi-pdf: patch
  takumi-raster: patch
---

# A box moved to the next page by `break-before: page` keeps its text

A paged PDF, or the pages `takumi-raster`'s `render_pages` streams, could lose the text of a box with `break-before: page` that came right after a flex container spanning several pages. The page that holds the box now matches the same document rendered in one pass.
