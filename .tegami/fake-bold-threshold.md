---
packages:
  "cargo:takumi": patch
  "cargo:takumi-core": patch
  "cargo:takumi-raster": patch
  "cargo:takumi-svg": patch
  "npm:takumi-js": patch
  "npm:@takumi-rs/core": patch
  "npm:@takumi-rs/wasm": patch
  "npm:@takumi-rs/image-response": patch
---

### Match browser fake-bold for synthesized weights

Synthesize bold only at the CSS bold threshold (weight >= 600), so a medium weight keeps its regular face instead of being faux-bolded. Scale the synthetic stroke with text size — `1/24` easing to `1/32` per Skia — instead of a constant fraction, so large text is no longer over-emboldened.
