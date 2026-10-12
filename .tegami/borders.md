---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Paint borders as Chrome does

- Sides of different colors or styles meet on the corner diagonal, with Chrome's miters and no seams.
- Rounded borders with mixed sides clip each side and cut corners as Chrome does.
- Dashes start at the corners.
- `dotted` up to 3px wide draws square dots that end on a whole dot. Wider ones draw round dots.
- `inset`, `outset`, `groove` and `ridge` shade like Chrome and stay visible on very dark colors.
- A `double` side under 3px and a 1px `groove` or `ridge` paint solid.
- `corner-shape` values such as `bevel`, `scoop` and `notch` keep the border's thickness around the corner.
- A collapsed table border stays square, even with `border-radius`.
- `background-clip: border-area` follows dashed, dotted and double borders.
- PDF draws dashed and dotted sides next to sides of other styles.
