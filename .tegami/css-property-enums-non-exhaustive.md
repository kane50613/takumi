---
packages:
  "cargo:takumi-css": minor
---

### Mark the property-identifier enums `#[non_exhaustive]`

`LonghandId`, `ShorthandId`, `PropertyId`, and `StyleDeclaration` gain a variant
for every CSS property, so supporting a new property stays non-breaking across
1.0.
