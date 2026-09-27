---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Apply `vertical-align` to inline spans, as Chrome does

`vertical-align` on a `display: inline` span now moves its text, background and children, and grows the line to fit, as Chrome does. `sub` and `super` shift by the parent's font size, percentages refer to the element's own line height, and `middle` uses the parent font's x-height.
