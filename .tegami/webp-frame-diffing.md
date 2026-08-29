---
packages:
  "@takumi-rs/core":
    type: patch
---

### Encode only the changed region of each animated WebP frame

Animated WebP frames after the first now carry just the rectangle that changed since the previous frame, stored with the no-blend flag so it replaces the canvas. Animations with a small moving region over a static background come out much smaller and encode faster. Frames that change everywhere are unaffected, and `dispose: true` keeps full-canvas frames.
