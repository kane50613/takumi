---
packages:
  npm:takumi-js:
    type: patch
---

### Fall back to the WASM backend in a WebContainer

Unbundled runs (e.g. `nitro dev` externalizing the package) resolve `#backend`
with Node's default conditions, so the native addon wins even when the host set
`unwasm`. The node backend now detects `process.versions.webcontainer` and loads
the WASM backend instead.
