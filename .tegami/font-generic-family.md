---
packages:
  "cargo:takumi-core": minor
  "npm:@takumi-rs/core": minor
  "npm:@takumi-rs/wasm": minor
  "npm:@takumi-rs/helpers": minor
---

### Claim generic font families from the JS font API

Font descriptors accept `generic` (e.g. `"monospace"`), so stacks like Tailwind's `font-mono` resolve to registered fonts without naming the family.
