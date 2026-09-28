---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Paint backgrounds as Chrome does

- `background-repeat: space` spreads the leftover room so the first and last tiles touch the edges, and a single tile follows `background-position`. `round` rounds the tile count to the nearest whole number. Both keep tiling across a painting area larger than the positioning area.
- Shorter `background-size`, `-position`, `-repeat` and `-blend-mode` lists cycle over the layers instead of repeating their last value.
- SVG places tiles at exact positions and positions `background-clip: text` layers by `background-origin`. PDF no longer repeats a layer along an axis that does not repeat.
- `background-blend-mode` in the image output blends only with the box's own layers and color, not with what sits behind the box.
- A repeating gradient whose stops all sit at one position paints solid in the last stop's color.
