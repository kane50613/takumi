---
packages:
  cargo:takumi-core:
    replay:
      - "exit prerelease: cargo:takumi-core"
---

### Keep the `color` and `selectors` crates out of the public API

`ColorInterpolationMethod`'s color-space fields and `CssRule`'s selector list
leaked the `color` and `selectors` crates. Their fields are now `pub(crate)`,
`build_color_lut_with_interpolation` takes the opaque interpolation method
instead of raw `color` types, and `CssRule` exposes a `selectors()` accessor.
