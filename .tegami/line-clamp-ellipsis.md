---
packages:
  "takumi": patch
---

# End clamped text with an ellipsis

`line-clamp` and `-webkit-line-clamp` now end the last line with an ellipsis when they cut text, as Chrome does, without `text-overflow: ellipsis`. Tailwind's `line-clamp-*` utilities show the `…` they show in a browser. `line-clamp: 2 none` cuts without one.
