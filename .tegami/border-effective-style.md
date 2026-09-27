---
packages:
  "takumi": patch
---

# Draw thin double, groove and ridge borders solid

A `double` border side under 3px and a `groove` or `ridge` side of 1px now paint solid, as Chrome paints them. Before, they split into slivers too thin to show the style.
