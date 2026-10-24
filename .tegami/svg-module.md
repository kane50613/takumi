---
packages:
  takumi-core: patch
---

# Gather the SVG code in one module

takumi-core keeps the SVG render tree, text layout, filters, and drawing ops under `svg`, and the vendored `resvg` module is gone.
