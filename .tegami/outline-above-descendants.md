---
packages:
  takumi-svg:
    type: patch
  takumi-raster:
    type: patch
---

### Paint an outline above the box's children

A box with a negative `outline-offset` drew its outline under its own children, so a ring dragged inside the box disappeared behind them. The outline now paints after everything the box contains.
