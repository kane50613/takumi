---
packages:
  "takumi": minor
---

# Blur shadows faster

The horizontal pass of the alpha box blur slides its window four pixels at a time with NEON, SSE2, or wasm simd128.
