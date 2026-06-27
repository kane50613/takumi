---
packages:
  cargo:takumi-css:
    replay:
      - "exit prerelease: cargo:takumi-css"
---

### Mark the property-identifier enums `#[non_exhaustive]`

`LonghandId`, `ShorthandId`, `PropertyId`, and `StyleDeclaration` gain a variant
for every CSS property, so supporting a new property stays non-breaking across
1.0.
