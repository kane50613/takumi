---
packages:
  npm:takumi-js:
    type: minor
---

### Select the WASM backend under the `unwasm` condition

Nitro sets `unwasm` on every preset, including Node, so `takumi-js` resolves
`#backend` to WASM there instead of the native addon a WebContainer host
(StackBlitz, CodeSandbox) can't load. Set `exportConditions: ["!unwasm"]` to keep
the native bindings on the Node preset.

The `module` escape hatch now goes through `#backend` instead of loading the WASM
glue from the shared graph, so it stays a lazy chunk on a native build.
