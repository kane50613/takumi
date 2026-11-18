---
packages:
  "@takumi-rs/wasm": patch
---

# Vite's dev server finds the WebAssembly binary for a bundled SSR dependency

When `ssr.noExternal` bundles `@takumi-rs/wasm` into the dev server's SSR modules, the first render now finds the WebAssembly binary in the package. It used to fail with this error:

```text
Unable to locate Takumi WASM asset for SSR: /node_modules/@takumi-rs/wasm/pkg/takumi_wasm_bg.wasm
```
