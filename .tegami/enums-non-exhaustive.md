---
packages:
  cargo:takumi-core:
    replay:
      - exit-prerelease(cargo:takumi-core)
---

### Mark public enums `#[non_exhaustive]`

The node and image enums (`NodeKind`, `ImageSource`, `ImageSourceInput`,
`ImageCacheMode`), the property identifiers (`LonghandId`, `ShorthandId`,
`PropertyId`, `StyleDeclaration`), and the spec-tracking value enums (`BlendMode`,
`Filter`, `BasicShape`, `ContentValue`, `TextTransform`, `WhiteSpaceCollapse`,
`OffsetPath`, `ImageScalingAlgorithm`, `Position`) can gain variants without a
breaking change. Match them with a wildcard arm.
