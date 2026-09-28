---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Paint borders as Chrome does

- Sides that differ in color or style meet along the corner diagonal, and adjacent sides of the same color leave no seam.
- A dashed side runs the full length of the box, so its dashes start at the corners.
- A `dotted` border up to 3px wide draws square dots spaced one dot apart. Wider dotted lines keep round dots inside the box.
- `inset`, `outset`, `groove` and `ridge` darken the shadowed edges and keep the color on the lit ones. A very dark color lightens instead, so both edges stay visible.
- PDF draws dashed and dotted sides next to sides of other styles, which it used to leave out.
- `background-clip: border-area` no longer paints a translucent border's color twice in the image output.
