---
packages:
  "npm:takumi-js": patch
---

### Resolve the render backend through import conditions

A `#backend` import map now selects napi on Node/Bun and WASM on workers, edge,
and browsers at resolve time, replacing the runtime global sniffing and
`@vite-ignore`d dynamic imports. Bundlers no longer drag the native
`@takumi-rs/core` binary into worker/edge output, and `@takumi-rs/wasm` resolves
under pnpm's strict layout.
