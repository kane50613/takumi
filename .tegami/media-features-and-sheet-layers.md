---
packages:
  takumi-core: patch
---

# More media features match, and cascade layers order across stylesheets as in Chrome

- `@media` reads `pointer`, `any-pointer`, `hover`, `any-hover`, `prefers-reduced-transparency`, `scripting`, `update`, `color`, `color-index`, `monochrome`, `color-gamut`, `dynamic-range`, `display-mode`, `grid`, `overflow-block`, `overflow-inline`, and the `device-` size features. A render answers as an 8-bit sRGB screen with no pointer, and print answers `update: none`, `overflow-block: paged`, and `overflow-inline: none`, as Chrome prints. Container queries still ignore these features.
- A size feature such as `(width: 800)` no longer matches with a unitless number other than `0`, and `(grid = 0)` no longer matches, as in Chrome. Number features read a `calc()`, so `(color: calc(8))` matches.
- Layers with the same name in separate stylesheets, such as an inline SVG's `<style>` and the page's CSS, are one layer, ordered where it first appears.
- A layer declared only inside an `@media` that does not match takes no place in the layer order.
