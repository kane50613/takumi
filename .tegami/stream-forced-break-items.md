---
packages:
  takumi-core: patch
  takumi-pdf: patch
  takumi-raster: patch
---

# A streamed render keeps the text of a box pushed past a forced break

When a render streamed its pages and a box with `break-before: page` came right after a flex container that ran onto a page of its own, the layout of the page before laid the box out and then moved it to the next page. That next page reused the layout, but the text inside the box was freed once the page before was drawn, so the page could differ from the same document rendered at once. The text now stays until the page that holds the box is drawn.
