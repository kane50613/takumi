---
packages:
  "cargo:takumi-core": minor
  "cargo:takumi-raster": minor
  "cargo:takumi-svg": minor
---

### Support SVG filter references in `filter`

`filter` and `backdrop-filter` accept `url(data:image/svg+xml,...)` with inline `<filter>` markup, mixing freely with filter functions. The raster backend executes the graph through resvg; the SVG backend emits the markup verbatim.
