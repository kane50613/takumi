---
packages:
  takumi-core:
    type: patch
  takumi-raster:
    type: patch
---

### Bound allocations and loops driven by untrusted input

Three denial-of-service paths are closed. SVG rasterization and canvas allocation now cap at 16M pixels and return an error past it, instead of aborting on a huge allocation. A `background-size` past `i32::MAX` no longer wraps a repeat step negative and loops forever.
