---
packages:
  "npm:@takumi-rs/wasm": patch
---

### Fix the Node bundler entries for ESM and CJS

The `node` and `bun` entries imported `../dist/export` without a file extension,
which Node's ESM resolver rejects with `ERR_MODULE_NOT_FOUND`; they now import
`../dist/export.mjs`. The `./auto` export also gains a `require` branch that
resolves to `node.cjs`, so `require("@takumi-rs/wasm/auto")` works on every
supported Node version instead of only those that can `require` an ES module.
