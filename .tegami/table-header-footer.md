---
packages:
  "takumi-core": minor
  "takumi-pdf": minor
  "takumi-html": patch
  "@takumi-rs/helpers": patch
---

# Repeat table headers and footers on every page, as Chrome prints them

- A `tfoot` no taller than a quarter of the page repeats below the last row on each page. The structure tree keeps one `TFoot`.
- A header or footer group repeats only with `break-inside: avoid`. The `thead` and `tfoot` presets set it, as Chrome's print stylesheet does.
