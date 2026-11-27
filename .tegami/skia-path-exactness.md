---
packages:
  takumi-core: minor
---

# Closed rects and very long dashed paths stroke as in Chromium

Strokes of closed rects and of paths with too many dashes now match Chromium's outlines.

- A closed rect strokes to an outer rect grown by half the stroke width with its corners mitered, bevelled, or rounded, and an inner rect where the stroke leaves a hole. Edges of such strokes can shade a colour level differently.
- A dashed path that would cut into more than a million dashes now strokes without dashes instead of drawing nothing.

`takumi_core::path` gains `Path::rect_contour`, which returns the rect a path of lines traces as a `RectContour`.
