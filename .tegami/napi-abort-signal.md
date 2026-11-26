---
packages:
  "@takumi-rs/core": patch
---

# An abort signal keeps its `onabort` handler

A `signal` passed to a render keeps the `onabort` handler you set on it. Aborting the signal now behaves as follows:

- Your `onabort` handler runs. It used to be replaced, so it never ran.
- Every render still waiting on the signal stops, even when an earlier render with the same signal has already finished.

```ts
controller.signal.onabort = cleanup;
await renderer.render(node, { width: 1200, height: 630, signal: controller.signal });
controller.abort();
// Before: `cleanup` never runs
// After: `cleanup` runs
```
