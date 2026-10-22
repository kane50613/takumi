---
packages:
  takumi-core: patch
---

# Paint a block inside an inline box as Chrome does

A block inside an inline box, such as a `<div>` in a `<span>`, now paints its background before the line's inline boxes, and its positioned boxes stack with the rest of the page. A percentage height inside it now resolves against its containing block.
