---
packages:
  takumi-core: patch
---

# Hide images an ellipsis cuts off

Since 2.17.0, an inline image at or past a `text-overflow: ellipsis` cut was still painted, over the ellipsis or in its place. It is now left out, and an ellipsis that lands right after an image paints as well.
