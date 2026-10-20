---
packages:
  takumi-core: minor
  takumi-html: minor
  takumi-pdf: patch
  "@takumi-rs/helpers": minor
---

# Lay out tables with Chromium's table algorithm

Tables now follow Chromium's table layout instead of a grid approximation, so column widths, row heights, and collapsed borders match Chromium. `<col>` and `<colgroup>` size their columns and paint their backgrounds, and `visibility: collapse`, `empty-cells: hide`, and `display: inline-table` work. A `rowspan` now stops at the end of its row group.
