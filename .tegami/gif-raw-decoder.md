---
packages:
  "cargo:takumi-core": patch
---

### Decode GIF frames on the raw `gif` decoder

Composite frames on one reused canvas instead of the `image` crate's per-frame allocations; skipped frames no longer allocate or premultiply.
