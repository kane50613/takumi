---
packages:
  takumi-pdf:
    type: patch
---

# PDFs draw damaged color fonts, unsorted fonts, and zero-length lines correctly

A COLR color font with a palette index past its palette, or a layer with no clip, no longer crashes the render, and its colors come from the font's first palette. A font whose table directory is out of order now embeds, a zero-length SVG line with `stroke-linecap: round` draws a dot, and an image's ICC profile embeds whenever the PDF version supports that profile version.
