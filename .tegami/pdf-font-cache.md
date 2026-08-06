---
packages:
  takumi-pdf:
    type: minor
---

### Convert each font once per renderer

Converting a font for PDF output copies its whole blob and hashes it, and it was happening on every render. A `PdfRenderer` now keeps the converted fonts, so the second document onwards skips the work. A CJK invoice renders in 1.8 ms instead of 2.9 ms.
