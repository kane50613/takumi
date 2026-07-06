---
packages:
  npm:@takumi-rs/core: patch
---

### Resolve the native binding under isolated installs

The bundled loader now finds `@takumi-rs/core-<target>` when it lives in a pnpm
or bun store rather than a hoisted `node_modules`, so strict installs no longer
throw `Cannot find module '@takumi-rs/core-<target>'`.
