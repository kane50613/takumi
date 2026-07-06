---
packages:
  cargo:takumi-core: minor
  cargo:takumi-raster: minor
---

### Seal remaining `taffy`/`parley` leaks out of the public `style` API

`Position`, `Display`, `AlignItems`, `JustifyContent`, `BoxSizing`, `Direction`,
`FlexDirection`, `FlexWrap`, `Overflow`, `TextAlign`, `GridPlacement`,
`GridAutoFlow`, `GridRepetitionCount`, and `GridTemplateAreas` no longer
implement `From<_>` for their `taffy`/`parley` counterparts; the conversions
are now `pub(crate)` inherent methods (`into_taffy`/`into_parley`).
`TextWrapMode`, `WordBreak`, and `OverflowWrap` lose their `parley` `From`
impls the same way. `ResolvedVerticalAlign::apply` is now `pub(crate)`.

`takumi-raster`: dropped the unused `From<OutputFormat> for image::ImageFormat`
impl, which pinned the `image` crate into the public API without being called
anywhere.
