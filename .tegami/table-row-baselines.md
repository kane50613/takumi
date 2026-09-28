---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Align table cells on their row's baseline, as Chrome does

Cells with `vertical-align: baseline` now line their first lines up on the row's deepest baseline, whatever their fonts, padding or borders. An element inside a row that is not a table cell now sits in an anonymous cell, as CSS table fixup puts it.
