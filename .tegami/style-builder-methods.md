---
packages:
  takumi-core: minor
---

# Set any style property with a builder method

`Style` now has one builder per longhand and shorthand, so `Style::default().font_size(32.0.px())` replaces `Style::default().with(StyleDeclaration::font_size(...))`. The `LengthUnit` trait adds `px`, `percent`, `em`, `rem`, `vw`, and `vh` to `f32`, each returning a `Length`. The `Style::with_<shorthand>` methods are deprecated in favor of the shorthand's builder of the same name.
