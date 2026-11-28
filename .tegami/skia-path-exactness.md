---
packages:
  takumi-core: minor
  takumi-pdf: patch
---

# Closed rects and dashed paths stroke closer to Chromium

A closed rect strokes as Chromium strokes it, into an outer rect with mitered, bevelled, or rounded corners and an inner rect where the stroke leaves a hole, so its edges can shade a colour level differently. A dashed stroke with extremely many tiny dashes draws as a solid stroke instead of disappearing, and a PDF form around a dashed stroke covers a square cap that reaches past a corner. `takumi_core::path` gains `Path::rect_contour`, which returns the rect a path of lines traces as a `RectContour`.
