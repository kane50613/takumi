---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Paint border sides the way Chrome does

Borders whose sides differ in color or style now match Chrome more closely in every output format:

- Sides meet along the corner diagonal instead of one side taking the whole corner square.
- A dashed side runs the full length of the box, so its dashes start at the corners.
- A dotted side keeps its end dots inside the box.

Adjacent sides of the same color no longer leave a faint seam at the corner.
