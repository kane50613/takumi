---
packages:
  "takumi-core":
    type: patch
---

### Round normal line heights like browsers

`line-height: normal` resolves from the primary font as
`round(ascent) + round(descent) + round(line gap)`, as Blink computes it, so
line stacks match browser pixel positions. Fallback-font runs still grow the
line to their own rounded height. Subset groups expand in registration order
when ranks tie, so the primary font is the first registered subset.
