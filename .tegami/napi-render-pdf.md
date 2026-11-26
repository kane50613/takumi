---
packages:
  "@takumi-rs/core": minor
---

# `Renderer` renders PDFs natively

`renderPdf` renders a node tree, JSX, or HTML to PDF bytes with the native binding, and `preparePdf` readies a document's fonts and images first, as `prepare` does for images. Both take the options `takumi-pdf` takes and write the same bytes.

```ts
import { Renderer } from "@takumi-rs/core";

const renderer = new Renderer();
const pdf = await renderer.renderPdf(<Invoice />, { size: "a4", footer: <PageFooter /> });
```
