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

A Tailwind v4 source stylesheet now works in `stylesheets` without compiling it first. `@theme` declarations land on `:root`, and `@keyframes` inside the block register. Modifiers like `reference` read the same way. The `prefix()` modifier is not supported.
