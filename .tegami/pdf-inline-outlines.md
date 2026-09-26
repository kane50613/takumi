---
packages:
  "takumi-pdf": patch
---

# Draw `outline` on inline elements in PDF

An `outline` on an inline element, such as a `<span>`, used to be left out of the PDF. It now strokes around the element's text as one contour across line breaks, at the element's opacity, as the image and SVG outputs already did.
