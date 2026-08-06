---
packages:
  takumi-pdf:
    type: minor
---

### Measure a tree without rendering

`measure` returns a tree's laid-out size in CSS px. With page options it lays out at the full page width and fills counter hooks with three-digit numbers, exactly how `render` measures a header or footer band, so the height tells you how much margin the band needs.
