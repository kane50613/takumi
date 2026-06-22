---
packages:
  "npm:@takumi-rs/core": patch
---

### Make font reads wait-free under concurrent renders

One lock guarded all renderer state, so every `registerFont` blocked in-flight
renders. Fonts now sit behind an `ArcSwap`, and the image cache moves out of the
outer lock. Concurrent render-and-register throughput rises 30–50% with lower
tail latency; single-threaded rendering is unchanged.
