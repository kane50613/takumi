---
packages:
  "@takumi-rs/image-response": patch
  "@takumi-rs/wasm": patch
---

# Editors point imports of `@takumi-rs/image-response` and `@takumi-rs/wasm/vite` to their replacements

Editors now flag imports from `@takumi-rs/image-response`, which only re-exports `takumi-js/response`, and from `@takumi-rs/wasm/vite`, which resolves to the same loader as `@takumi-rs/wasm/auto` under Vite. Both keep working and will be removed in v3.
