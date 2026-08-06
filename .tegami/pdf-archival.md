---
packages:
  takumi-pdf:
    type: minor
---

### PDF/A and tagged output

Output is now tagged by default, like Chromium's print-to-PDF. The structure tree comes from the HTML semantics: headings, paragraphs, figures with alt text, links, and header/footer artifacts. Decorative `<img alt="">` images are artifacts. `tagged: false` turns the tree off. `tagged: "ua1"` validates against PDF/UA-1.

Set `pdfa` to a level from `"2b"` to `"4"` to emit archival PDFs with an sRGB output intent and XMP metadata. The `a` levels require the structure tree and `metadata.creationDate`. A document that cannot conform rejects the render instead of writing a broken file.
