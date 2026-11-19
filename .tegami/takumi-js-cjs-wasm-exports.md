---
packages:
  takumi-js: patch
---

# `require("takumi-js/wasm")` keeps the exports it re-exports

A CommonJS `require("takumi-js/wasm")` returned only the backend's default `init`, so the names `takumi-js/wasm` re-exports from `@takumi-rs/wasm`, such as `Renderer`, were missing. It now returns them beside `default`, as the ES module and the type declarations already did.
