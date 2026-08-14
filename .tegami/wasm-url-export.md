---
packages:
  "@takumi-rs/wasm":
    type: minor
  takumi-js:
    type: minor
  takumi-pdf:
    type: minor
---

### Load the wasm binary in a browser bundle

Vite, webpack and Turbopack set the same export conditions for a browser build. All three resolved the Vite entry, whose `?url` import only works in Vite. Each package now exports `wasm-url`, which resolves the binary through `new URL(specifier, import.meta.url)`. Pair it with `takumi-pdf/no-init`, or with the new `takumi-js/wasm/no-init`, which keeps the auto-init entry out of the bundle.
