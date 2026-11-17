---
packages:
  takumi-pdf:
    type: patch
---

# PDFs draw damaged color fonts, unsorted fonts, and zero-length lines correctly

A PDF render handles these fonts and shapes as Chromium's print output does:

- A COLR color font with a palette index past its palette, or a layer with no clip, no longer crashes the render. A glyph it cannot draw is left out instead.
- COLR colors come from the font's first palette, so a font whose palettes do not start at the first color record draws its intended colors.
- A font whose table directory is out of order embeds and subsets, where its tables were not found before.
- An SVG line of zero length with `stroke-linecap: round` draws a dot.
- An image with an ICC profile embeds its profile when the PDF version supports that profile version, such as a v2.4 profile in PDF 1.7, which was refused before.
