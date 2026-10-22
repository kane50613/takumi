---
packages:
  takumi-core: patch
---

# Size tables inside flex and grid containers as Chrome does

A table that is a flex item or a grid item now takes the size its container gives it, as Chrome sizes it. A flex item table no longer treats its `height` or `min-height` as a minimum when it flexes. A grid item table fills an explicitly sized grid area and keeps to its `max-width` and `max-height`.
