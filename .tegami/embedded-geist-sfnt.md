---
packages:
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
  takumi-pdf: patch
  takumi-paint: patch
---

# Faster renderer initialization

Renderers now load the bundled variable Geist font without decompressing WOFF2 or copying its bytes. Custom WOFF2 fonts remain supported.
