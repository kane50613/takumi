---
packages:
  "npm:@takumi-rs/wasm": patch
---

### Fix the Node ESM bundler entry

The `node` and `bun` entries imported `../dist/export` without a file extension,
which Node's ESM resolver rejects with `ERR_MODULE_NOT_FOUND`. They now import
`../dist/export.mjs`, so `@takumi-rs/wasm/node` and the `node` branch of
`@takumi-rs/wasm/auto` load in a plain Node process.
