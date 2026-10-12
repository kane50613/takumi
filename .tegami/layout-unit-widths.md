---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Measure content widths as Chrome does

- A box sized to its content rounds its text width up to 1/64px, as Chrome's `LayoutUnit` does, not to a whole pixel.
- A shrink-to-fit box whose text wraps, such as a flex item, takes the width it wraps at.
- `width`, `height` and `flex-basis` ignore `fit-content(<length-percentage>)`, which Chrome allows only in grid tracks.
