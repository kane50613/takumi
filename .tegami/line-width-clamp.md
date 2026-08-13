---
packages:
  takumi-core:
    type: minor
---

### Round `border-width` and `outline-width` to whole pixels

Both now resolve the way Blink does. A width under 1px becomes 1px, so a thin border no longer fades in and out with its position. Anything else rounds down, so `1.5px` renders as `1px`.
