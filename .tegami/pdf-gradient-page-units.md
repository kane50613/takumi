---
"takumi-pdf": patch
---

# Scale gradient transforms to the page unit

A gradient inside a tiling pattern was built in pixels while its matrix resolves against the page in points, so every gradient drew 4/3 too large and a radial or conic one also landed off-centre.
