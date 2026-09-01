---
packages:
  "takumi-pdf":
    type: patch
  "@takumi-rs/wasm":
    type: patch
---

### Build client apps with `noExternal` on the Vite entry

The Vite entry no longer imports `node:fs/promises` with a literal specifier,
so client builds that bundle dependencies stop failing with
`Cannot bundle Node.js built-in "node:fs/promises"`.
