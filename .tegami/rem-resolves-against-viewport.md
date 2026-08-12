---
packages:
  takumi-core:
    type: minor
---

### Resolve `rem` against the viewport instead of the outermost node

`rem` used the computed `font-size` of the tree's outermost node, so a `text-2xl` there silently scaled every `rem` length below it, including the whole Tailwind spacing scale. `rem` and `rlh` now resolve against the viewport's font size. A tree rooted at an `<html>` element keeps the old basis, since CSS makes that element the root; the JS `fromHtml` returns one for a full document.
