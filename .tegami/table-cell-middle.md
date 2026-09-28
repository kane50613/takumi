---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Center table cells by default, as Chrome does

The HTML presets now give `thead`, `tbody` and `tfoot` Chrome's `vertical-align: middle`, which rows and cells inherit. A cell that holds only text now takes its `vertical-align` too.
