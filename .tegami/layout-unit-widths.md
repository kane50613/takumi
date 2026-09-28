---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Measure content widths as Chrome does

A box sized to its content now takes its text's width rounded up to a 64th of a pixel, as Chrome's `LayoutUnit` does, rather than to a whole pixel. A shrink-to-fit box whose text wraps, such as a flex item or `width: fit-content(120px)`, now takes the width it wraps at rather than its widest line.
