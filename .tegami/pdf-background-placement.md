---
packages:
  takumi-pdf:
    type: minor
---

### Place background layers

`background-size`, `background-position` and `background-repeat` now apply to gradient layers, which used to stretch across the whole box whatever those properties said.

- `repeat`, `space` and `round` become one PDF tiling pattern per layer, so a repeated gradient costs one shading rather than one per tile
- the positioning area is still the border box; `background-origin` is not read yet
