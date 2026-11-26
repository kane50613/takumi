---
packages:
  "@takumi-rs/core": patch
---

# An abort signal keeps its `onabort` handler

Passing a `signal` to a render used to replace the signal's `onabort` handler with the binding's own. Each native task now follows the signal through a signal of its own, so the caller's handler still runs. An abort also cancels a render queued after a font-subset read that already finished on the same signal.

```ts
controller.signal.onabort = cleanup;
await renderer.render(node, { width: 1200, height: 630, signal: controller.signal });
controller.abort();
// Before: `cleanup` never runs
// After: `cleanup` runs
```
