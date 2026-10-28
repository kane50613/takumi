---
packages:
  takumi-pdf: minor
  takumi-paint: minor
  "@takumi-rs/helpers": minor
---

# `PdfRenderer` and `Painter` prepare a document too

`PdfRenderer.prepare` and `Painter.prepare`, and the shared `prepare` each package exports, fetch the images a document draws and load the font subsets its text uses, as `Renderer.prepare` does. A PDF's header, footer, and page bands count as part of the document. `render`, `measure`, and `paint` take the prepared document, a PDF with only file options such as `metadata`, `outline`, and `pdfa`.

With every renderer able to prepare, `extractEmojis` and the `node` option of `prepareImages` are deprecated. Pass `emoji` to `prepare`, which also reads the images a stylesheet names.
