---
packages:
  "@takumi-rs/core":
    type: patch
---

### Accept CSS-wide keywords on shorthand properties

`margin: inherit`, `padding: initial` and `border: unset` were rejected. Only longhands took CSS-wide keywords. A shorthand now expands the keyword across the longhands it targets.
