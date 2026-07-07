---
packages:
  npm:@takumi-rs/core: patch
---

### Restore statically analyzable native binding requires

The isolated-install fallback replaced the loader's literal
`require('@takumi-rs/core-<target>/core.<target>.node')` calls with a
dynamically built specifier, so Turbopack, `@vercel/nft`, and `bun build
--compile` could no longer trace or embed the native binding. The loader
now tries the literal require first and only falls back to store scanning
when it fails. A `bun` export condition routes Bun to the CJS build,
whose requires Bun's bundler can embed into compiled binaries.
