---
packages:
  "npm:takumi-js": patch
  "npm:@takumi-rs/core": patch
  "npm:@takumi-rs/wasm": patch
  "npm:@takumi-rs/image-response": patch
---

### Fix `workspace:*` leaking into the published `package.json`

Published packages shipped their inter-package dependencies as the literal
`workspace:*` range, so installing them failed with `Workspace dependency
"@takumi-rs/core" not found`. The publish step now resolves `workspace:` ranges
to concrete versions, matching what `bun` and `pnpm publish` already do.
