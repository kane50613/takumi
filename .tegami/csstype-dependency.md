---
packages:
  "npm:@takumi-rs/core": patch
  "npm:@takumi-rs/wasm": patch
---

### Declare csstype as a runtime dependency

The published type definitions import `csstype`, but it was only a
devDependency, so consumers hit `Cannot find module 'csstype'`. Move it to
`dependencies`.
