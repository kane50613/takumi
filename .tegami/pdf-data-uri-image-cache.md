---
packages:
  takumi-pdf: minor
---

# PDF renders decode a repeated `data:` URI image once

A PDF that repeats a `data:` URI image, such as a table with the same signature in every row, used to decode and parse the image each time layout looked at it, so long documents rendered slowly. `PdfOptions` and `MeasureOptions` in the `takumi-pdf` crate take a `resource_cache` as the raster renderer's options do, and a render without one keeps its own. `PdfRenderer` passes its cache, so its renders share decoded images.
