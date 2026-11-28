---
packages:
  takumi-core: minor
---

# SVG stroke caps, joins, and spread methods import from new paths

Code that names the SVG stroke or gradient enums through `takumi_core::resources::image` imports them from their shared homes. The variants keep their names.

- `SvgLineCap` and `SvgLineJoin` are gone. `SvgStrokeStyle` holds `takumi_core::style::LineCap` and `LineJoin`.
- `SvgSpreadMethod` is now `takumi_core::paint::SpreadMethod`.

#### Migration

```diff
- use takumi_core::resources::image::{SvgLineCap, SvgLineJoin, SvgSpreadMethod};
+ use takumi_core::{
+   paint::SpreadMethod,
+   style::{LineCap, LineJoin},
+ };

- let cap = SvgLineCap::Round;
- let join = SvgLineJoin::Bevel;
- let spread = SvgSpreadMethod::Reflect;
+ let cap = LineCap::Round;
+ let join = LineJoin::Bevel;
+ let spread = SpreadMethod::Reflect;
```
