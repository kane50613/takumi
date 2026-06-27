---
packages:
  "cargo:takumi-css": minor
---

### Encapsulate foreign-typed public fields behind accessors

`ColorInterpolationMethod`'s `color_space`/`hue_direction` and `CssRule`'s
`selectors` were public fields exposing the `color` and `selectors` crates. They
are now `pub(crate)` with accessor methods.
