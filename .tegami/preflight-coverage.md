---
packages:
  "@takumi-rs/core":
    type: minor
  "@takumi-rs/wasm":
    type: minor
  "takumi-pdf":
    type: minor
---

### Cover the rest of Preflight

`@import "tailwindcss"` used to reset margins, padding, heading fonts and list markers. It now carries the rest of Preflight that a renderer can act on: the universal border reset, `line-height: 1.5`, link and table resets, `small` and `sub`/`sup` sizing, and block-level images. Rules for elements takumi never renders stay out.
