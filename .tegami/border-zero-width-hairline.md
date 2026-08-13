---
packages:
  takumi-core:
    type: patch
---

### Stop drawing a hairline on borderless sides

A border on only some sides drew faint lines along the sides that had none, in PDF and in SVG. Those sides now draw nothing.
