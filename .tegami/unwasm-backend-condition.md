---
packages:
  npm:takumi-js:
    type: patch
---

### Select the WASM backend under the `unwasm` condition

Nitro sets `unwasm` on every preset, including Node, so `takumi-js` resolves
`#backend` to WASM there instead of the native addon a WebContainer host
(StackBlitz, CodeSandbox) can't load. Set `exportConditions: ["!unwasm"]` to keep
the native bindings on the Node preset.
