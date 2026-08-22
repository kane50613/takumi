---
packages:
  "@takumi-rs/core":
    type: patch
---

### Draw animated frames without replaying the ones before them

A frame that covers the whole canvas and replaces what is under it does not depend on the frames before it, so it is now decoded on its own. Reaching the last frame of a 300-frame animation drops from 16.6ms to 0.13ms for GIF, 158ms to 0.39ms for APNG, and 97ms to 0.05ms for WebP. Frames that do blend onto their predecessors still replay, as they must.
