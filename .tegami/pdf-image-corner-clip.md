---
packages:
  "takumi-pdf": patch
---

# Trim an image to its content edge curve

A `border-radius` on an image was ignored unless the box also clipped its overflow, so a rounded picture came out square.
