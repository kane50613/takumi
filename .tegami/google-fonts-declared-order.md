---
packages:
  npm:@takumi-rs/helpers: patch
---

### Fix `googleFonts` losing your declared family order

`googleFonts` returned subsets in whatever order Google's `css2` response happened to list
`@font-face` blocks in — not the order families were passed in `families`. A render with no
explicit `fontFamilies` falls back to registration order, so a Han-unified codepoint shared by
two requested families (e.g. `"Noto Sans TC"` and `"Noto Sans JP"`) could pick the wrong one
regardless of how `families` was ordered. `googleFonts` now sorts its result to match the
caller's declared order.
