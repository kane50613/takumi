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

Preflight used to strip declarations off the UA preset, which cost `<b>`, `<strong>`, `<small>`, `<sub>`, `<sup>` and `<th>` their font sizing and weight. It is now a real stylesheet in its own layer, so only `h1` through `h6` give those up and every author rule still outranks it.
