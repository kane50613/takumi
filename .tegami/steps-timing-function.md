---
packages:
  "@takumi-rs/core":
    type: patch
---

### Support the full `steps()` step position syntax

`steps()` now accepts `jump-start`, `jump-end`, `jump-none`, and `jump-both`, and the position argument is optional, so `steps(4)` means `steps(4, jump-end)`. Only `start` and `end` parsed before, so a declaration like `steps(4, jump-none)` was dropped as invalid and the animation fell back to `ease`, drawing a smooth curve instead of a staircase.
