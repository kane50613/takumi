---
packages:
  cargo:takumi-core:
    replay:
      - exit-prerelease(cargo:takumi-core)
---

### Mark spec-tracking CSS value enums `#[non_exhaustive]`

`BlendMode`, `Filter`, `BasicShape`, `ContentValue`, `TextTransform`,
`WhiteSpaceCollapse`, `OffsetPath`, `ImageScalingAlgorithm`, and `Position` can
gain variants as their specs grow without a breaking change.
