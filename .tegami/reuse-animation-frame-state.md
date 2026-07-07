---
packages:
  "cargo:takumi-raster": patch
  "cargo:takumi": patch
  "npm:@takumi-rs/core": patch
  "npm:@takumi-rs/wasm": patch
---

### Reuse per-scene state across animation frames

Compute each scene's font snapshot once and share its image and stylesheet
handles across frames instead of re-snapshotting and deep-cloning the whole
option tree per frame. Frame output is unchanged.
