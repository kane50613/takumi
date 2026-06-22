---
packages:
  "cargo:takumi": patch
  "cargo:takumi-core": patch
  "npm:takumi-js": patch
  "npm:@takumi-rs/core": patch
  "npm:@takumi-rs/wasm": patch
  "npm:@takumi-rs/image-response": patch
---

### Expand subset font-family without borrowing the font context

A render holds the font context borrowed while it builds text layout, so per-element
`font-family` skipped subset-group expansion and routing relied on the fallback bucket, which
followed `fontique`'s hash iteration order. Content registered with `subset_of` (Rust) or
`subsetOf` (JS) could route to the wrong subset, and the result changed between renders.
Expansion now reads the subset groups without that borrow, and the fallback bucket follows
registration order.
