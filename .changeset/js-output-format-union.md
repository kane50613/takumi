---
"@takumi-rs/core": major
"@takumi-rs/wasm": major
"takumi-js": major
---

Model the output format as a discriminated union so `quality` and `lossless` only appear on the formats that accept them, and clamp out-of-range WebP quality instead of throwing
