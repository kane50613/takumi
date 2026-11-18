---
packages:
  "@takumi-rs/core": patch
---

# A missing native binding names the package to install

When `@takumi-rs/core` cannot load its native binding, the error now names the platform package to install and links the troubleshooting guide, instead of blaming an npm bug. On a platform with no published binding, it points to `@takumi-rs/wasm`.
