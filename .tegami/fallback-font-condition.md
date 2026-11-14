---
packages:
  "@takumi-rs/wasm": minor
  "@takumi-rs/helpers": minor
  takumi-paint: minor
  takumi-pdf: minor
---

# Leave the built-in Geist out of a WebAssembly bundle

The WebAssembly packages now import Geist from `@takumi-rs/helpers/fallback-font` instead of building it into the binary. Set the `takumi-no-fallback-font` export condition in your bundler and that module resolves to an empty one, which saves about 25 KB (brotli) when you load your own fonts. Without the condition, renders are unchanged. `@takumi-rs/core` still builds Geist in.
