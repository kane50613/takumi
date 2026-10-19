---
packages:
  takumi-core: patch
---

# Parse `flex: none` as `0 0 auto`

`flex: none` now keeps a flex item from growing or shrinking, as CSS defines it. It used to reset the item to `0 1 auto`, so the item still shrank to fit its container.
