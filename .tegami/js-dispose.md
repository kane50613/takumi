---
packages:
  "@takumi-rs/wasm": minor
  takumi-pdf: minor
  takumi-paint: minor
---

# A `using` block frees a renderer

`Renderer`, `PdfRenderer` and `Painter` implement `Symbol.dispose`, so `using renderer = new Renderer()` frees the renderer's WebAssembly memory when the block ends, as calling `free()` does.
