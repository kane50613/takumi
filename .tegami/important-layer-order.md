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

An important `tw` utility used to beat every important author rule. The cascade reverses layer order for important declarations, and `tw` is the last declared layer, so it now loses to them and still wins against their normal half. Inline important declarations stay on top.
