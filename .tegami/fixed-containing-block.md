---
packages:
  takumi-core:
    type: patch
---

### Let a transform or filter capture the boxes it contains

`position: fixed` resolved against the viewport no matter what it sat inside, and `position: absolute` only looked for a positioned ancestor. A `transform`, `translate`, `rotate`, `scale`, `offset-path`, `filter` or `backdrop-filter` now makes a box the containing block for both, the way browsers do.
