---
packages:
  "takumi-core": patch
  "takumi-pdf": patch
  "takumi-html": patch
  "@takumi-rs/helpers": patch
---

# Repeat a table header or footer only when it avoids breaks, as Chrome does

A header or footer group repeats on every page only with `break-inside: avoid`, which the `thead` and `tfoot` presets now set like Chrome's print stylesheet. A tree that sets `display: table-header-group` without it no longer repeats the group.
