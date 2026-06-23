---
packages:
  "npm:@takumi-rs/wasm": patch
---

### Resolve the SSR WASM asset without guessing the output dir

The Vite bundler entry mapped the `?url` asset to disk by guessing a `client/`
directory, which broke dev (`/@fs/` URLs) and frameworks with a different layout
(e.g. Waku's `public/`). It now reads the asset colocated with the server chunk
via `import.meta.url`, with the `client/` paths kept as fallbacks.
