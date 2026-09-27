---
packages:
  "takumi": patch
---

# Resolve `clip-path` shapes the same way in every output format

A percentage corner radius in `clip-path: inset(... round ...)` now resolves its vertical radius against the box height in the SVG output, as it already did in the image output. A `path()` clip that cannot be parsed no longer hides its element in the image output.
