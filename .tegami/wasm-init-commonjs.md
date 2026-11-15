---
packages:
  takumi-js: patch
  "@takumi-rs/wasm": patch
---

# `require` loads the WebAssembly backend again

A CommonJS build that rendered on the WebAssembly backend, such as one in a WebContainer, failed with `TypeError: (0 , _takumi_rs_wasm.default) is not a function`. The `init` that `require("takumi-js/wasm")` and `require("takumi-js/wasm/no-init")` return was the module object instead of a function. Both now work, and `@takumi-rs/wasm` exports `init` by name as well as by default.
