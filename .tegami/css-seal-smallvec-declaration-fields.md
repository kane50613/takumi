---
packages:
  cargo:takumi-core: minor
---

### Keep `smallvec` and `image` out of the declaration-block API

`StyleDeclarationBlock::declarations` and `DeclarationImportance::custom_properties`
exposed `smallvec::SmallVec` as public fields; they are now `pub(crate)` with
`len`/`is_empty` accessors alongside the existing `iter`. Dropped the unused
`From<ImageScalingAlgorithm> for image::imageops::FilterType`, removing `image`
from the value-enum surface.
