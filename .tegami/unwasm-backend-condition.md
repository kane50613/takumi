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

Neither backend reaches into the other's bundle now: the native backend rejects a
WASM `module`. To force WASM on a native runtime, pass a `renderer` from
`createRenderer` (`takumi-js/wasm`).
