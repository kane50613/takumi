---
packages:
  "@takumi-rs/core":
    type: patch
  "@takumi-rs/wasm":
    type: patch
  "takumi-pdf":
    type: patch
---

### Resolve `--breakpoint-*` overrides by cascade layer

A `--breakpoint-*` declaration inside `@layer` used to beat an unlayered one that came before it. Overrides now follow the cascade: unlayered wins over any layer, and a later layer wins over an earlier one.
