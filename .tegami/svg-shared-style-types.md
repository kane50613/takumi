---
packages:
  takumi-core: minor
---

# SVG strokes and spread methods use the shared style types

`SvgStrokeStyle` now holds `style::LineCap` and `style::LineJoin`, so `SvgLineCap` and `SvgLineJoin` are gone. `SvgSpreadMethod` is now `takumi_core::paint::SpreadMethod`, since CSS gradients and PDF colour glyphs use it too. The variants keep their names.

#### Migration

```diff
- use takumi_core::resources::image::{SvgLineCap, SvgLineJoin, SvgSpreadMethod};
+ use takumi_core::{
+   paint::SpreadMethod,
+   style::{LineCap, LineJoin},
+ };
```
