---
packages:
  takumi-core:
    type: minor
---

### Round `border-width` and `outline-width` to whole pixels

Both now resolve the way Blink does: a width under 1px becomes 1px, and anything else rounds down to a whole CSS pixel. A hairline that used to fade in and out with its position now draws as a solid 1px line, and `1.5px` renders as `1px`.
