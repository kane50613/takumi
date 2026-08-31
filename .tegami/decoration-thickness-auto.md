---
packages:
  "takumi-core":
    type: patch
---

### Match browser thickness for text-decoration auto

`text-decoration-thickness: auto` resolves to a tenth of the font size with a
1px floor, as Blink computes it; `from-font` keeps the font's own metric.
