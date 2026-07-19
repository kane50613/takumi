---
packages:
  "cargo:takumi-core": patch
---

### Ellipsize nowrap text without a break opportunity

`text-overflow: ellipsis` with `white-space: nowrap` only kicked in when the text could wrap, so a single long token was clipped with no ellipsis. Overflow detection now also checks the line's inline advance against the box, matching how Blink truncates any overflowing line at a character boundary.
