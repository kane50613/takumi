---
packages:
  takumi-core: minor
---

# Closed rects and very long dashed paths stroke as in Chromium

Strokes of closed rects and of paths with too many dashes now match Chromium's outlines.

- A closed rect strokes to an outer rect grown by half the stroke width with its corners mitered, bevelled, or rounded, and an inner rect where the stroke leaves a hole. Edges of such strokes can shade a colour level differently.
- A dashed stroke with extremely many tiny dashes now draws as a solid stroke instead of disappearing, as in Chromium.

`takumi_core::path` gains `Path::rect_contour`, which returns the rect a path of lines traces as a `RectContour`.
