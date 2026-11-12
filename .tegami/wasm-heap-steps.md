---
packages:
  "@takumi-rs/wasm":
    type: patch
  takumi-pdf:
    type: patch
  takumi-paint:
    type: patch
---

# Long documents render faster in WebAssembly

The WebAssembly builds now grow their memory an eighth at a time, at least 16 MiB, in place of one allocation's worth. Each growth costs V8 about 100 µs, and a long document needed tens of thousands of them. An 11,000-row PDF renders in 29 s in place of 35 s, with the same bytes, and small renders are unchanged.
