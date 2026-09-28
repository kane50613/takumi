---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Place absolutely positioned boxes as Chrome does

- An `auto`-width absolute box wraps its content to the containing block's width minus its insets and margins, so `left: 50%` text no longer runs past the right edge.
- An absolute box inside a paragraph keeps the text around it on one line and starts where it sits in that line.
- An absolute box inside a `position: relative` span takes its offsets from that span's box, and no longer lets a line break inside the word around it.
