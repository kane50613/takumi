---
packages:
  takumi-pdf:
    type: minor
---

### File attachments

Attach files with `attachments`, the PDF/A-3 shape ZUGFeRD and Factur-X e-invoices use. `data` takes bytes or a UTF-8 string. `modificationDate` falls back to `metadata.creationDate`. Invalid `pdfa` combinations are TypeScript type errors. Without type checking they reject the render at runtime.
