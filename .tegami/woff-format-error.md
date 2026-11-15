---
packages:
  takumi-core: patch
  "@takumi-rs/wasm": patch
---

# A WOFF font in the WebAssembly bindings names the format it cannot read

Registering a WOFF font in a build without WOFF support, such as `@takumi-rs/wasm`, failed with `Unsupported font format`. It now fails with `This build cannot read WOFF fonts`, which says to convert the font, and the docs list the formats each package reads.
