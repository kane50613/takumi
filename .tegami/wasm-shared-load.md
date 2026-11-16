---
packages:
  takumi-js: patch
---

# Overlapping first renders load the WebAssembly binary once

When `render` and `ImageResponse` calls overlap before the WebAssembly binary has loaded, they now share one load. Before, each call instantiated the binary, and the later one broke the renderers made on the earlier one. If the load fails, the next call tries again.
