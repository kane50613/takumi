---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Place absolutely positioned boxes as Chrome does

- An `auto`-width absolute box wraps to the space its insets and margins leave, so `left: 50%` text stays inside the edge.
- An absolute box inside a paragraph starts where it sits in the line, and the text around it stays on one line.
- An absolute box inside a `position: relative` span takes its offsets from the span and no longer breaks the word around it.
