---
packages:
  "takumi-core": minor
  "takumi-pdf": minor
---

# Repeat a table's footer on every page it breaks across

A `tfoot` no taller than a quarter of the page now paints again below the last row of every page the table's body breaks across, as Chrome prints it, and each such page saves room for it. The repeats are artifacts, so the structure tree keeps one `TFoot`.
