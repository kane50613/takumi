---
packages:
  npm:@takumi-rs/core: patch
  npm:@takumi-rs/wasm: patch
---

### Accept a promise in the `fonts` option

`fonts` now takes `Promise<FontLoader[]>` as well as the plain list, so
`googleFonts` results pass straight through without `await`.
