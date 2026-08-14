---
packages:
  takumi-core:
    type: patch
  takumi-pdf:
    type: patch
---

### Draw glyphs through a type-erased pen

skrifa stamps out its whole CFF/glyf evaluator per concrete pen type. Routing every outline draw through one `ErasedPen` drops ~110KB from the wasm binaries with identical rendering.
