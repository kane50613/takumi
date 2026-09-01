---
packages:
  "takumi-pdf":
    type: patch
  "@takumi-rs/wasm":
    type: patch
  "takumi-js":
    type: patch
  "@takumi-rs/image-response":
    type: patch
---

### Resolve a browser-only entry in client builds

Bundlers that resolve the `browser` condition (Vite client, webpack web) now
get `bundlers/browser.mjs`, which only fetches the `.wasm` asset by `import.meta.url`. Client builds
with `noExternal` stop failing with `Cannot bundle Node.js built-in "node:fs/promises"`.
The Vite server entry reads the asset through `process.getBuiltinModule`, so
no bundler sees a Node import. These packages, plus `takumi-js` and `@takumi-rs/image-response` on top of them, now require Node 20.19 or newer.
