---
packages:
  takumi-pdf:
    type: patch
---

# A font with a non-finite CFF number no longer crashes a PDF render

A CFF font that holds `NaN` or an infinite real number, as a damaged font file can, made `takumi-pdf` panic while it subset the font. The PDF now renders, and the number is written as `0`. A font matrix that overflows once scaled to the CID font's units now fails the subset, instead of writing a matrix that hides every glyph.
