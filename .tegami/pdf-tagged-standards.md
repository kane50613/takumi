---
packages:
  takumi-pdf:
    type: minor
---

### Tagged PDF: PDF/UA-1 and the PDF/A `a` levels

`pdfua: true`, `pdfa: "2a"` and `pdfa: "3a"` build a structure tree from the HTML semantics: headings, paragraphs, figures with alt text, links, and header/footer artifacts. `tagged: true` builds the tree without enforcing a standard, and `metadata.creationDate` sets the document date the `a` levels require.
