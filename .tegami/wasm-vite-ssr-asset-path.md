---
packages:
  "@takumi-rs/wasm":
    type: patch
---

### Find the WASM asset when Vite emits it beside the server bundle

A plain `vite build --ssr` with `ssrEmitAssets` writes the asset to `assets/` under the same `outDir` as the server chunk. The SSR read looked next to the chunk and in a framework's `client/` directory, missed both, and threw `Unable to locate Takumi WASM asset for SSR`.
