---
packages:
  "takumi": patch
---

# Fit text with `text-fit` before adding an ellipsis

With `text-overflow: ellipsis`, a `text-fit` line no longer paints at full size outside its box. A line that fits once scaled gets no ellipsis, and a line that still overflows gets one at its scaled size.
