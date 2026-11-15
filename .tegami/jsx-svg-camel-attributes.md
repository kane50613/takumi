---
packages:
  "@takumi-rs/helpers": patch
---

# JSX SVG keeps `preserveAspectRatio` and `clipPathUnits`, and writes `dominantBaseline` as SVG spells it

`fromJsx` wrote `preserveAspectRatio` and `clipPathUnits` as `preserve-aspect-ratio` and `clip-path-units`, which SVG ignores, so a JSX `preserveAspectRatio="none"` had no effect. It left `dominantBaseline`, `strikethroughPosition`, `strikethroughThickness`, `renderingIntent`, and `xHeight` in camelCase, which SVG ignores too. Each attribute is now written the way React DOM writes it.
