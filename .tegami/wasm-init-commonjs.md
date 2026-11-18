---
packages:
  takumi-js: patch
  "@takumi-rs/wasm": patch
---

# `require` loads the WebAssembly backend again

A CommonJS build that renders on the WebAssembly backend, such as one in a WebContainer, works again. It used to fail with `TypeError: (0 , _takumi_rs_wasm.default) is not a function`.

```js
const { init } = require("takumi-js/wasm");
// Before: init is the module object, not a function
// After: init loads the WebAssembly binary
```

`require("takumi-js/wasm/no-init")` returns the same `init`, and `@takumi-rs/wasm` now exports `init` by name as well as by default.
