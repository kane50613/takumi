---
packages:
  "npm:@takumi-rs/helpers": patch
---

### Raise the default fetch timeout to 30 seconds

`AbortSignal.timeout` counts wall-clock time, so a 5-second budget aborted otherwise-healthy font fetches whenever heavy synchronous work (SSG, wasm rendering) blocked the event loop past it.
