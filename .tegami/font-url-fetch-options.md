---
packages:
  "@takumi-rs/core":
    type: patch
  "@takumi-rs/wasm":
    type: patch
  takumi-pdf:
    type: patch
  takumi-paint:
    type: patch
  takumi-js:
    type: patch
---

# Download fonts listed as URLs with the render's fetch options

A URL string in a render's `fonts` list now downloads with the `fetch`, `allowUrl`, `timeout`, `maxBytes`, and `signal` the render's images and `@font-face` files use, so `allowUrl` refuses it too. With a custom `fetch`, it skips the renderer's default store, as images do. A render that waits on another render's download of the same font tries its own download when that one fails.
