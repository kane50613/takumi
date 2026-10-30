---
packages:
  "@takumi-rs/core": minor
  "@takumi-rs/wasm": minor
  takumi-pdf: minor
  takumi-paint: minor
---

# `prepare` downloads the fonts `@font-face` rules load

An `@font-face` rule in `css` or a `<style>` element now loads its font in `prepare`, `render`, and `ImageResponse`. A `src` that fails to download falls back to the next one.
