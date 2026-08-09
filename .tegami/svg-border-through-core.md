---
packages:
  takumi-core:
    type: minor
  takumi-svg:
    type: patch
---

### Draw every border ring from the shared painter

The SVG backend draws its borders through the shared painter. A `double` border fills two rings instead of stroking two centerlines.
