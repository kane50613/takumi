---
packages:
  takumi-core:
    type: minor
  takumi-raster:
    type: patch
  takumi-pdf:
    type: patch
  takumi-svg:
    type: patch
---

# An SVG image no longer takes the host's color, as in Chromium

An SVG drawn by `<img>`, `background-image`, or another image draws `currentColor` as its own `color`, or black when it sets none, because Chromium draws each SVG image as a document of its own. Only an `svg` element takes the `color` of the box it sits in: an inline `<svg>` in HTML, a JSX `<svg>`, or a `Node` whose tag name is `svg`. To keep an icon in the host's color, render it as an `svg` element, or give a `Node::image` the tag name with `.with_tag_name("svg")`.
