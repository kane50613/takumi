---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Paint borders as Chrome does

- Sides that differ in color or style meet along the corner diagonal, and adjacent sides of the same color leave no seam.
- A dashed side runs the full length of the box, so its dashes start at the corners.
- A `dotted` border up to 3px wide draws square dots spaced one dot apart and ends each side on a whole dot. Wider dotted lines keep round dots inside the box.
- `inset`, `outset`, `groove` and `ridge` darken the shadowed edges and keep the color on the lit ones. A very dark color lightens instead, so both edges stay visible.
- PDF draws dashed and dotted sides next to sides of other styles, which it used to leave out.
- `background-clip: border-area` keeps the background only where a dashed, dotted or double border paints, and no longer paints a translucent border's color twice in the image output.
- Square sides with mixed styles, colors or opacities meet with Chrome's miters. A uniform dashed or dotted border shows no diagonal seam, and translucent sides no longer blend twice where they overlap.
- Rounded borders with mixed colors, styles or opacities clip each side and cut their corners as Chrome does.
- A `double` side under 3px and a 1px `groove` or `ridge` side paint solid.
- A collapsed table border stays square even with `border-radius`, as the spec requires.
