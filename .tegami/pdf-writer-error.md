---
packages:
  takumi-pdf:
    type: minor
---

# `PdfError::Krilla` is now `PdfError::Writer`

The variant that carries a failure from writing the PDF file is renamed, since the PDF writer is now takumi's own module. Match on `PdfError::Writer` where you matched on `PdfError::Krilla`. Its message is unchanged.
