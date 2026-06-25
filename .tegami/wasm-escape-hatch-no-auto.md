---
packages:
  "npm:takumi-js": patch
---

### Keep the Vite WASM loader out of Node bundles

The WASM escape hatch always carries an explicit `module`, so it now loads
`wasm-init` directly instead of `@takumi-rs/wasm/auto`. A Next/webpack node
build that only uses napi no longer drags the Vite `?url` binary loader into
its graph, where the unresolvable query broke the build.
