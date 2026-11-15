---
packages:
  "@takumi-rs/wasm": patch
---

# Vite's dev server finds the WebAssembly binary for a bundled SSR dependency

When `ssr.noExternal` bundled `@takumi-rs/wasm` into the dev server's SSR modules, the first render failed with `Unable to locate Takumi WASM asset for SSR: /node_modules/@takumi-rs/wasm/pkg/takumi_wasm_bg.wasm`. The loader now reads the binary from the package in that case too.
