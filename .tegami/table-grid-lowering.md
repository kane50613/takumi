---
packages:
  "@takumi-rs/core":
    type: minor
  "@takumi-rs/helpers":
    type: minor
---

### Lay out tables on the grid algorithm

A `<table>` used to fall back to block layout, so cells stacked instead of forming columns. Table boxes now lower onto a grid whose column tracks are shared by every row. Header groups render first, footer groups last. `colspan` and `rowspan` span tracks, captions render on the side `caption-side` picks, and a row's background lands on its cells. HTML and JSX gain element presets for `table`, `thead`, `tbody`, `tfoot`, `tr`, `td`, `th`, and `caption`.
