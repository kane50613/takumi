---
packages:
  "takumi": minor
---

# Finish opaque renders faster

The final premultiplied-to-straight alpha pass skips 16-pixel runs that are fully opaque or fully transparent, using NEON, SSE2, AVX2, or wasm simd128 to find them.
