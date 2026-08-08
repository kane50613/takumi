---
packages:
  takumi-pdf:
    type: minor
---

### Validate against PDF/UA-2

`tagged: "ua2"` validates the structure tree against PDF/UA-2. The standard is written against PDF 2.0, so it pairs with `pdfa: "4"` and `pdfa: "4f"`, or with plain PDF, and the types reject the PDF 1.7 levels. The `Document` element now carries the PDF 2.0 namespace, which PDF/UA-2 requires of every structure element.

Links and outline entries pointing inside the document still target a page position rather than a structure element, so veraPDF reports rule 8.8-1 on a document with headings or internal links.
