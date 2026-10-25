---
packages:
  takumi-core: patch
  takumi-svg: patch
  takumi-pdf: patch
---

# Isolate an SVG group with `isolation: isolate`

A `<g style="isolation: isolate">` inside an SVG image or inline `<svg>` now composites its content as one group, as in Chrome, so a descendant's `mix-blend-mode` blends only with what the group paints instead of with the page behind it.
