---
packages:
  cargo:takumi-core:
    replay:
      - exit-prerelease(cargo:takumi-core)
---

### Hide internal style/layout items from the public API

Roughly 200 CSS value types, parsing helpers, and internal accessors across `style::properties`,
`style::stylesheets`, `style::tw`, `style::selector`, and `layout` are now crate-private; they were
never meant to be constructed or matched on directly. `StyleSheet::property_rules` and
`apply_stylesheet_animations` are crate-private; use `StyleSheet`'s public parsing/query surface
instead. Gradient direction/keyword helpers (`GradientKeywordDirection`, `HorizontalKeyword`,
`VerticalKeyword`) stay public since they're reachable through `LinearGradientDirection`.
