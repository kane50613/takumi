---
packages:
  "@takumi-rs/core": minor
  "@takumi-rs/wasm": minor
---

# `Renderer.needs` reports what a render will fetch

`needs(source, options)` takes a node tree or HTML and returns the remote image URLs it draws and the characters its text shapes, read after the cascade. A stylesheet's `background-image`, generated content, and `text-transform` count, and a `display: none` subtree does not.
