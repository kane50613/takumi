---
packages:
  takumi-pdf:
    type: minor
  takumi-core:
    type: patch
---

### Render HTML and CSS list markers in PDF

PDF output now paints generated markers for `ul`, `ol`, and `display: list-item`, including nested and paginated lists, custom counter styles, inside/outside positioning, and marker images. Tagged PDFs place visible labels in `Lbl` alongside each item's `LBody`, and generated marker characters participate in JavaScript font subsetting.

The `square` counter style now generates `▪` (U+25AA BLACK SMALL SQUARE), matching the CSS Counter Styles definition browsers use, instead of the oversized `■`.
