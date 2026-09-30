---
packages:
  "takumi": minor
---

# Paint borders on inline spans

A `display: inline` span now paints its `border` around its text on each line, and reserves the border's width at its start and end. A span that wraps leaves out the side at each break, as browsers do by default.
