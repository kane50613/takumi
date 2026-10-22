---
packages:
  takumi-core: minor
---

# Center a block's content with `align-content`

`align-content: center` and `end` now move the content of a block container with a fixed height, as Chrome does, and `space-around` and `space-evenly` center it. The content never moves above the top, and a container with an `align-content` other than `normal` becomes a block formatting context. `flow-start` and `flow-end` now parse in `align-content` and `justify-content`.
