---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Tile background and mask layers the way browsers do

`background-repeat: space` now spreads the leftover room so the first and last tiles touch the edges, and a single tile follows `background-position` instead of centering. `round` counts tiles to the nearest whole number, and both keep tiling across a painting area larger than the positioning area. Shorter `background-size`, `-position`, `-repeat`, and `-blend-mode` lists now cycle over the layers instead of repeating their last value. The SVG backend places tiles at exact positions, and PDF no longer repeats a layer along an axis that does not repeat.
