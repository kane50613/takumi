---
packages:
  takumi-js: minor
---

# `ready()` loads the WebAssembly binary for `takumi-js/wasm`

`await ready()` from `takumi-js/wasm` resolves once the bindings have loaded the binary the export conditions pick, on every runtime, so a `Renderer` can be created after it. Before, the default export was a function, a compiled module, or a promise of bytes depending on the runtime, and code had to check which one it got. `ready()`, `render`, and `ImageResponse` now share one load, so overlapping calls no longer instantiate the binary twice.
