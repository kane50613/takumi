---
packages:
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
  takumi-paint: patch
  takumi-pdf: patch
---

# `unregisterFont` leaves the built-in Geist in place

`unregisterFont` now removes only the fonts you registered. Before, unregistering `Geist` also removed the built-in Geist, so text that no registered font covered drew nothing.

```ts
// @takumi-rs/core and @takumi-rs/wasm. Use `new Painter()` in takumi-paint and `new PdfRenderer()` in takumi-pdf.
const renderer = new Renderer();

// Before: true, and the built-in Geist is gone
// After: false, since no registered font is named Geist
renderer.unregisterFont({ name: "Geist" });
```
