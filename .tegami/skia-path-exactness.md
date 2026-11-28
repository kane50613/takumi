---
packages:
  takumi-core: minor
  takumi-pdf: patch
---

# Closed rects and dashed paths stroke closer to Chromium

Strokes of closed rects and of dashed paths now follow Chromium more closely, in raster and PDF output.

- A closed rect strokes to an outer rect grown by half the stroke width with its corners mitered, bevelled, or rounded, and an inner rect where the stroke leaves a hole. Edges of such strokes can shade a colour level differently.
- A dashed stroke with extremely many tiny dashes now draws as a solid stroke instead of disappearing, as in Chromium.
- A PDF form around a dashed stroke also covers a square cap that reaches past the solid outline at a corner.

`takumi_core::path` gains `Path::rect_contour`, which returns the rect a path of lines traces as a `RectContour`.
