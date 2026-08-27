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

Preflight now carries every rule a renderer can act on. That adds the universal border reset, link and table resets, and block-level images. Rules for elements takumi never renders stay out.
