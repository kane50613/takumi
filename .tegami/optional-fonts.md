---
packages:
  "@takumi-rs/helpers":
    type: minor
  "@takumi-rs/core":
    type: minor
  "@takumi-rs/wasm":
    type: minor
  takumi-pdf:
    type: minor
  takumi-paint:
    type: minor
  takumi-js:
    type: minor
---

# Render without a font that fails to load when the font is optional

A font in a render's `fonts` list takes `optional: true`. When it fails to download, `allowUrl` refuses it, or its bytes do not decode, the render goes on without it instead of rejecting, as an `@font-face` rule moves on past a failed `src`. `fontFromUrl(url, { optional: true })` marks a URL optional. `fonts` also takes a `readonly` array.
