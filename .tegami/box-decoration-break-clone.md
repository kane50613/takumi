---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Repeat a `box-decoration-break: clone` span's edges on every line

An inline span with `box-decoration-break: clone` now draws its border, padding and corner radii on every line it wraps onto, and each line makes room for the repeated start edge, as in Chrome. Before, the wrapped lines kept `slice`'s open edges.
