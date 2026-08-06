---
packages:
  takumi-pdf:
    type: minor
---

### File attachments

Attach files with `attachments`: name, bytes or UTF-8 string, mime type, description, and an `AFRelationship`. Combined with `pdfa: "3b"` this produces the PDF/A-3 shape that ZUGFeRD and Factur-X e-invoices use; `modificationDate` falls back to `metadata.creationDate` to keep output deterministic. Invalid combinations are TypeScript type errors: the PDF/A-2 levels and `"4"` forbid attachments, and the PDF/A-3 levels require a mime type and description on each one.
