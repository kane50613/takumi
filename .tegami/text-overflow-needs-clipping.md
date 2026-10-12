---
packages:
  "takumi": minor
---

# Apply `text-overflow` only to a box that clips

`text-overflow: ellipsis` needs `overflow` other than `visible`, as in Chrome. Add `overflow: hidden` where you relied on an ellipsis without it. `line-clamp` still adds its own.
