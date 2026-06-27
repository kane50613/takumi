---
packages:
  cargo:takumi-core:
    replay:
      - 'exit prerelease: cargo:takumi-core'
---

### Mark the core node and image enums `#[non_exhaustive]`

`NodeKind`, `ImageSource`, `ImageSourceInput`, and `ImageCacheMode` can now gain
variants without a breaking change, so the surface stays stable across 1.0.
