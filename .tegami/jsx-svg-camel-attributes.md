---
packages:
  "@takumi-rs/helpers": patch
---

# `preserveAspectRatio`, `clipPathUnits`, and `dominantBaseline` take effect on a JSX `<svg>`

`fromJsx` now writes each attribute of an inline `<svg>` the way React DOM writes it. Before, SVG ignored the attributes below, so `preserveAspectRatio="none"` had no effect.

```tsx
<svg viewBox="0 0 10 10" preserveAspectRatio="none">
  …
</svg>
// Before: written as preserve-aspect-ratio="none", which SVG ignores
// After: written as preserveAspectRatio="none"
```

- `preserveAspectRatio` and `clipPathUnits` stay in camelCase, as SVG spells them.
- `dominantBaseline`, `renderingIntent`, `strikethroughPosition`, `strikethroughThickness`, and `xHeight` become `dominant-baseline`, `rendering-intent`, `strikethrough-position`, `strikethrough-thickness`, and `x-height`.
