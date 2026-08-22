---
packages:
  "@takumi-rs/core":
    type: patch
---

### Start backwards-filled step animations on the first step

An animation with `animation-fill-mode: backwards` or `both` and a positive delay held the second step during the delay when its timing function was `steps(n, jump-start)` or `steps(n, jump-both)`. Samples taken before the active phase now land on the step below the jump, as the CSS easing algorithm requires.

`steps()` also serializes the way the spec asks: `steps(4, end)` and `steps(4, jump-end)` both print as `steps(4)`, since an end position is the default.
