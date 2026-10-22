---
packages:
  takumi-core: minor
---

# Center a block's content with `align-content`

`align-content: center` and `end` now move the content of a block container with a fixed height, as Chrome does, and `space-around` and `space-evenly` center it. Content that overflows stays at the top unless the value says `unsafe`, as in `unsafe end`. A container with an `align-content` other than `normal` becomes a block formatting context. `align-content` and `justify-content` now keep `unsafe` and accept `flow-start` and `flow-end`. Table cells honor `unsafe` too.
