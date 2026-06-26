---
packages:
  "npm:@takumi-rs/core": patch
---

### Ship the native platform packages

Publish the per-platform `@takumi-rs/core-*` packages and inject their
`optionalDependencies` during release. Installs once again resolve a native
binary instead of falling back to the WASM escape hatch.
