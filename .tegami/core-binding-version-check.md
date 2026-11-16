---
packages:
  "@takumi-rs/core": patch
---

# `@takumi-rs/core` fails to load a native binary from another release

`@takumi-rs/core` now throws when its platform package, such as `@takumi-rs/core-linux-x64-gnu`, comes from another release, instead of rendering with it. A mismatched binary can misread what the package passes it. A 2.18 package on a 2.20 binary drew the node tree's JSON as text. Set `NAPI_RS_ENFORCE_VERSION_CHECK=0` to load the binary anyway.
