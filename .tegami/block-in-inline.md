---
packages:
  takumi-core: patch
---

# Keep an inline box inline when it holds a block

A block inside an inline box, such as a `<div>` in a `<span>`, now sits on a line of its own while the inline box keeps its lines before and after it, as in Chrome. The inline box used to turn into a block, so its background and border wrapped the whole thing.
