---
packages:
  takumi-js: minor
  "@takumi-rs/helpers": minor
---

# `render` fetches the images a stylesheet names

`render`, `measure`, `renderSvg`, and `renderAnimation` now ask the renderer which remote images the styled tree draws, then fetch those. A `background-image`, `mask-image`, or `list-style-image` set by `css`, a `<style>` element, or a `::before` rule now renders instead of being left out. An image under `display: none` is no longer fetched. A custom `renderer` from an older binding keeps the node-tree lookup. `prepareImages` also takes a `urls` list to fetch.
