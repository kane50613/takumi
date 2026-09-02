---
packages:
  takumi-core:
    type: minor
---

### Drop unused paint re-exports

`takumi_core::paint` no longer re-exports the tile position helpers, the gradient row states, or `LinearGradientFastPath`; a few layout helpers are crate-private now.
