---
packages:
  takumi-js: patch
---

# Overlapping first renders load the WebAssembly binary once

On the WebAssembly backend, `render` and `ImageResponse` calls that overlap before the WebAssembly binary has loaded now share one load. Before, each call loaded the binary, and the later load broke the renderers made on the earlier one. If the load fails, the next call tries again.

```ts
await Promise.all([
  render(<A />, { width: 1200, height: 630 }),
  render(<B />, { width: 1200, height: 630 }),
]);
// Before: each call loads the binary, and the first one's renderer can break
// After: both calls share one load
```
