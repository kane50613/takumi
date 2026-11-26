---
packages:
  takumi-core: patch
---

# `OffsetRotate::resolve` takes and returns degrees

`OffsetRotate::resolve` now takes the path's tangent in degrees and returns an `Angle`, so rotation along a path keeps the degrees you write, as Chrome does.

#### Migration

```diff
- let radians = offset_rotate.resolve(tangent_radians);
+ let radians = (*offset_rotate.resolve(tangent_radians.to_degrees())).to_radians();
```
