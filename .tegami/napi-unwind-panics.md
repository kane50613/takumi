---
packages:
  "@takumi-rs/core": patch
---

### Convert native panics into catchable errors instead of aborting the process

The published napi artifacts are now built with unwind panics, so a Rust panic reached through malformed input surfaces as a JS error rather than killing the host process. Wasm keeps abort panics by design.
