---
packages:
  "@takumi-rs/core":
    type: patch
  "@takumi-rs/wasm":
    type: patch
  "takumi-pdf":
    type: patch
---

### Keep font styles outside headings under Preflight

Preflight used to strip declarations off the UA preset. That cost `<b>`, `<strong>`, `<small>`, `<sub>`, `<sup>` and `<th>` their font sizing and weight. Only `h1` through `h6` give those up now, and every author rule outranks Preflight.
