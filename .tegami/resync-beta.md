---
packages:
  "npm:takumi-js": patch
  "npm:@takumi-rs/core": patch
  "npm:@takumi-rs/helpers": patch
  "npm:@takumi-rs/wasm": patch
  "npm:@takumi-rs/image-response": patch
---

### Re-release all packages in sync

Earlier beta releases drifted out of lockstep, so some published packages
depended on versions that were never published. Bump and publish the set
together so the beta tag is consistent and every inter-package dependency
resolves.
