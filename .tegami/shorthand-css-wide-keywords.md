---
packages:
  "@takumi-rs/core":
    type: patch
  "@takumi-rs/wasm":
    type: patch
  "takumi-pdf":
    type: patch
---

### Accept CSS-wide keywords on shorthand properties

`margin: inherit`, `padding: initial`, `border: unset` and every other shorthand paired with a CSS-wide keyword were rejected. Only longhands took them. A shorthand now expands the keyword across the longhands it targets.
