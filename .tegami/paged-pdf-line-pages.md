---
packages:
  takumi-pdf: patch
---

# Paged PDFs keep the text a transform or a clip moves

In a paged PDF, text in a rotated or scaled box went missing from every page after the first. A line that an `overflow: hidden` box or the page edge cut in half went missing too. Each now draws on the page that shows it, as Chromium prints it, and the PDF holds its text. A line a transform moves above the first page or below the last also draws, on that page.
