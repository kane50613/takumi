---
packages:
  "@takumi-rs/core":
    type: minor
  "@takumi-rs/wasm":
    type: minor
  "takumi-pdf":
    type: minor
---

### Resolve `animate-*` through CSS variables

`animate-spin` now reads `var(--animate-spin)` with the built-in animation as the fallback, and an unknown token like `animate-wiggle` reads `var(--animate-wiggle)` alone. Pair the variable with its `@keyframes` through `stylesheets` or the `keyframes` option.
