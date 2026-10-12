---
packages:
  "takumi": patch
---

# End clamped text with an ellipsis

`line-clamp` and `-webkit-line-clamp` end the last line with `…`, as Chrome does, so Tailwind's `line-clamp-*` looks as it does in a browser. `line-clamp: 2 none` cuts without one.
