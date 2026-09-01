---
packages:
  "takumi-pdf":
    type: patch
  "@takumi-rs/wasm":
    type: patch
---

### Resolve a browser-only entry in client builds

Bundlers that resolve the `browser` condition (Vite client, webpack web) now
get `bundlers/browser.mjs`, which only fetches the `.wasm` asset. Client builds
with `noExternal` stop failing with `Cannot bundle Node.js built-in "node:fs/promises"`.
The Vite server entry also hides its `node:fs/promises` import from bundlers
that skip the `browser` condition.
