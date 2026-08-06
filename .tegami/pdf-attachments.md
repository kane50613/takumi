---
packages:
  takumi-pdf:
    type: minor
---

### File attachments

Attach files with `attachments`: name, bytes or UTF-8 string, mime type, description, and an `AFRelationship`. Combined with `pdfa: "3b"` this produces the PDF/A-3 shape that ZUGFeRD and Factur-X e-invoices use; `modificationDate` falls back to `metadata.creationDate` to keep output deterministic.
