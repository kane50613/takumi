---
packages:
  takumi-js: patch
---

# Render on the native backend under Nitro's `!unwasm` opt-in

A Nitro build with `exportConditions: ["!unwasm"]` picked `@takumi-rs/core`, but every request failed with `TypeError: __exportAll is not a function`. It now renders with the native addon, and the `bun` preset no longer bundles the WASM binary next to it.
