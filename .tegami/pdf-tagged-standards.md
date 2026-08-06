---
packages:
  takumi-pdf:
    type: minor
---

### Tagged PDF: PDF/UA-1 and the PDF/A `a` levels

Output is now tagged by default, like Chromium's print-to-PDF: a structure tree built from the HTML semantics — headings, paragraphs, figures with alt text, links, and header/footer artifacts. `tagged: false` turns it off. `pdfua: true`, `pdfa: "2a"` and `pdfa: "3a"` validate the result, and `metadata.creationDate` sets the document date the `a` levels require.
