---
packages:
  "@takumi-rs/wasm": patch
---

# Type the `Renderer` methods' results again

2.17.0 declared `render`, `renderAsDataUrl`, `renderSvg`, `measure`, `renderAnimation`, and `registerFont` as returning `any`, so `takumi-js`'s `render()` and `measure()` inferred `any` too. They now declare their promised results, such as `Promise<Uint8Array<ArrayBuffer>>` for `render`.
