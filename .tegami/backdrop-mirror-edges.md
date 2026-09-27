---
packages:
  "takumi": patch
---

# Keep content outside a backdrop-filter box out of its blur

A blurred `backdrop-filter` now reads only the backdrop inside the element's box, mirrored across its edges, as Chrome does. Before, dark content just outside the box bled into its edges.
