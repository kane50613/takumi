---
packages:
  npm:@takumi-rs/core: patch
---

### Revert isolated-install native binding resolution

Drop the `.pnpm`/`.bun` store scan from the bundled loader; it broke the build.
The loader statically requires `@takumi-rs/core-<target>/core.<target>.node` again.
