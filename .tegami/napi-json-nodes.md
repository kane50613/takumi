---
packages:
  "@takumi-rs/core": patch
---

# Read node trees off the main thread

`render`, `renderSvg`, and `measure` pass a node tree to the native renderer as JSON when it holds only plain values. The renderer parses it on a worker thread, so a 48-card dashboard blocks the event loop for 0.1 ms instead of 0.7 ms and renders about 10% faster. Trees holding bytes or other values JSON cannot keep are read as before.
