---
packages:
  takumi-core:
    type: patch
---

### Collect a box's shadows once

`BoxPainter::shadows` resolves `box-shadow` and splits it into the layers inside the box and the ones outside, so a backend no longer walks the list itself.

A fully transparent shadow is dropped. Two backends used to keep it and paint nothing.
