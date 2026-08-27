---
packages:
  "@takumi-rs/core":
    type: patch
  "@takumi-rs/wasm":
    type: patch
  "takumi-pdf":
    type: patch
---

### Order important declarations by reversed layer order

An important `tw` utility used to beat every important author rule. It now loses to important rules in named `@layer`s and still beats unlayered ones. Inline important declarations stay on top.
