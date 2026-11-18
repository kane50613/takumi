---
packages:
  takumi-pdf:
    type: patch
---

# Damaged fonts and extreme transforms no longer crash a PDF render

`takumi-pdf` used to panic on a few inputs that a damaged font or an extreme style can produce. The PDF now renders in each case:

- A font whose italic angle is `NaN` embeds with an italic angle of `0` in its PDF font descriptor.
- A clip path whose transform overflows `f32` clips everything inside it.
- A glyph whose bounds come out `NaN` falls back to a unit box for its paint bounds.
