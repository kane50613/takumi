---
packages:
  takumi-core:
    type: patch
  takumi-svg:
    type: patch
  takumi-pdf:
    type: patch
---

### Paint the outline above the content

An `outline` painted under the box's own text and images, so a negative `outline-offset` disappeared behind them. CSS 2.1 Appendix E paints the outline last, and every backend now does.
