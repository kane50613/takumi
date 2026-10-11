---
packages:
  "takumi": minor
---

# Apply `text-overflow` only to a box that clips

`text-overflow: ellipsis` now takes effect only when the box's `overflow` isn't `visible`, as in Chrome. Add `overflow: hidden` where you relied on an ellipsis without it. `line-clamp` still ends clamped text with an ellipsis on its own.
