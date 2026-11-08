---
packages:
  takumi-core: patch
---

# Count child blocks in `line-clamp`, and clamp by height with `line-clamp: auto`

`line-clamp` used to count only the lines directly inside the clamped box, so text inside child `<div>`s or `<p>`s was never clamped. The lines of child blocks now count, and every box after the clamp point is hidden, as in Chrome. `line-clamp: auto` clamps after the last line that fits the box's `height` or `max-height`. `block-ellipsis` also takes `no-ellipsis`.
