---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Follow `corner-shape` curves on the inner border edge

Borders and padding-box clips on boxes with `bevel`, `scoop`, `notch` and other `corner-shape` values now keep one thickness around each corner, as Chrome draws them. Opposite concave corners that would overlap now shrink the way Chrome shrinks them.
