---
packages:
  takumi-pdf:
    type: minor
---

### Measure a tree without rendering

`measure` returns a tree's laid-out size in CSS px. With page options it lays out at the full page width with counter hooks filled, exactly how `render` measures a header or footer band. The height tells you how much margin the band needs.
