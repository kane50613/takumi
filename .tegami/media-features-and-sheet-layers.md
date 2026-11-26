---
packages:
  takumi-core: patch
---

# More media features match, and cascade layers order across stylesheets as in Chrome

- `@media` reads `pointer`, `any-pointer`, `hover`, `any-hover`, `prefers-reduced-transparency`, `scripting`, `update`, `color`, `color-index`, `monochrome`, `color-gamut`, `dynamic-range`, `display-mode`, `grid`, `overflow-block`, `overflow-inline`, and the `device-` size features. A render answers as an 8-bit sRGB screen with no pointer, and print answers `update: none`, `overflow-block: paged`, and `overflow-inline: none`, as Chrome prints. Container queries still ignore these features.
- Layers with the same name in separate stylesheets, such as an inline SVG's `<style>` and the page's CSS, are one layer, ordered where it first appears.
- A layer declared only inside an `@media` that does not match takes no place in the layer order.
