---
packages:
  "npm:@takumi-rs/wasm": patch
---

### Resolve `edge-light` in the `/auto` export

`@takumi-rs/wasm/auto` now maps the `edge-light` condition (Next.js / Vercel
Edge) to the `?module` loader, so edge bundlers get the binary form they need
instead of falling through to the Vite `?url` loader.
