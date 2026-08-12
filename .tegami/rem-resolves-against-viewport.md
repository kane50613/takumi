---
packages:
  takumi-core:
    type: minor
---

### Resolve `rem` against the viewport instead of the outermost node

`rem` used the computed `font-size` of the tree's outermost node, so a `text-2xl` there silently scaled every `rem` length below it, including the whole Tailwind spacing scale. A tree built in code is content rather than a document, so `rem` and `rlh` now resolve against the viewport's font size. A tree parsed from a full HTML document keeps the old basis, since its outermost node is the `<html>` element that CSS makes the root.
