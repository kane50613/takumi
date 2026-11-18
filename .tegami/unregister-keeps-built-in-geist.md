---
packages:
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
  takumi-paint: patch
---

# `unregisterFont` leaves the built-in Geist in place

`unregisterFont` now removes only the fonts you registered. Before, unregistering `Geist` also removed the built-in Geist, so text that no registered font covered drew nothing.

```ts
const renderer = new Renderer();

// Before: true, and the built-in Geist is gone
// After: false, since no registered font is named Geist
renderer.unregisterFont({ name: "Geist" });
```
