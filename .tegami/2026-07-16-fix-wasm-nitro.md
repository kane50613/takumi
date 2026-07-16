---
packages:
  "npm:@takumi-rs/wasm": patch
---

### Fix WebAssembly parsing on Bun/Nitro

Remove `--all-features` from the `wasm-opt` configuration to prevent generating experimental WebAssembly features (such as typed function references) that cause parsing errors (e.g. `unexpected value type 100`) in bundlers like Nitro/unwasm. Explicitly enable stable features (`bulk-memory`, `mutable-globals`, and `sign-ext`) instead.
