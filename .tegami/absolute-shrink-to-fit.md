---
packages:
  "takumi": patch
---

# Shrink absolutely positioned boxes to the room beside their insets

An absolutely positioned box with an `auto` width now wraps its content to the containing block's width minus its `left`, `right` and margins, as browsers do. Before, `left: 50%` text could run past the right edge instead of wrapping.
