---
packages:
  "takumi-paint": patch
---

# Keep less memory for each painted tree

A painted tree no longer keeps a second copy of itself in WebAssembly memory. Keeping several large trees at once takes about a third less memory.
