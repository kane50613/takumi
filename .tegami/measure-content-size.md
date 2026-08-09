---
packages:
  takumi-pdf:
    type: patch
---

### Report the measured tree's own width

`measure` handed back the width it laid the tree out against, so a box with `width: 100px` measured 793 on an A4 page. It now reports the size the tree itself took.
