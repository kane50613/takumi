---
packages:
  "@takumi-rs/wasm": patch
---

# `@takumi-rs/wasm` reads node trees faster

A node tree now crosses into wasm as JSON text, as it already does in `@takumi-rs/core`, instead of being read field by field from the JavaScript object. Reading a 5,000-node tree takes about 20 ms instead of 23 ms.
