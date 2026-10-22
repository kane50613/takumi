---
packages:
  "@takumi-rs/core": patch
---

# Copy font and image bytes before a worker thread reads them

`registerFont`, `render`, `renderSvg`, `measure`, and `renderAnimation` now copy the `Buffer`, `Uint8Array`, or `ArrayBuffer` they are given before work moves to a worker thread. Rewriting or transferring those bytes while the call runs no longer corrupts its result or reads freed memory.
