---
packages:
  takumi-js: minor
  "@takumi-rs/helpers": minor
---

# `render` fetches the images a stylesheet names

`render`, `measure`, `renderSvg`, and `renderAnimation` now fetch the remote images `Renderer.needs` reports after the cascade, so a `background-image`, `mask-image`, or `list-style-image` from `css`, a `<style>` element, or a `::before` rule renders instead of being left out, and an image under `display: none` is no longer fetched. A custom `renderer` without `needs` keeps the node-tree lookup. `prepareImages` takes `urls`, and its `node` option is deprecated.
