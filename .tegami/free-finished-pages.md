---
packages:
  takumi-raster: patch
  takumi-pdf: patch
---

# Stop a long document's memory from growing with its page count

`render_pages` and `render` free what each page held once they have painted it, so the memory a streamed document needs no longer grows with its page count. A document that lays out to its end before the first page is drawn is unchanged.
