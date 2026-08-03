---
packages:
  cargo:takumi-core: patch
---

### Paint fractional border widths evenly

Layout rounding snapped border widths to whole pixels per edge, so a uniform 2.5px border could come out as a 2px top and a 3px bottom depending on where each edge landed on the pixel grid. Border widths now keep their fractional values through layout rounding and paint with coverage antialiasing, matching how browsers draw them.
