---
packages:
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
  takumi-paint: patch
  takumi-pdf: patch
---

# Registering a font copies its bytes once

`registerFont` copies the font data into the binding once, where it used to copy it two or three times, so loading a large CJK font needs less memory at its peak. In `takumi-paint`, a font's `data()` reads its file once and returns the same bytes on every later call.
