---
packages:
  "@takumi-rs/core":
    type: minor
  "@takumi-rs/wasm":
    type: minor
  "takumi-pdf":
    type: minor
---

### Parse `@theme` blocks as `:root` rules

A Tailwind v4 source stylesheet now works in `stylesheets` without compiling it first. `@theme` declarations land on `:root`, `@keyframes` inside the block register, and `@theme reference` emits nothing. The `prefix()` modifier is not supported.
