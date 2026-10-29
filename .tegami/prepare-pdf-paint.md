---
packages:
  takumi-pdf: minor
  takumi-paint: minor
  "@takumi-rs/helpers": minor
---

# `PdfRenderer` and `Painter` prepare a document too

`PdfRenderer.prepare` and `Painter.prepare`, and the shared `prepare` each package exports, fetch the images a document draws and load the font subsets its text uses, a PDF's header, footer, and page bands included. `render`, `measure`, and `paint` take the prepared document, a PDF with only file options such as `metadata` and `pdfa`. The `node` option of `prepareImages` is deprecated in favor of `prepare`.
