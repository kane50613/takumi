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

An SVG drawn by `<img>`, `background-image`, or another image now draws `currentColor` in its own `color`, or black when it sets none. Chromium draws each SVG image as a document of its own, apart from the page. Only an `svg` element takes the `color` of the box it sits in:

- an inline `<svg>` in HTML
- a JSX `<svg>`
- a `Node` whose tag name is `svg`

```html
<div style="color: red">
  <!-- Red before and after -->
  <svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="10" fill="currentColor" /></svg>
  <!-- The same circle as a file. Before: red. After: black -->
  <img src="circle.svg" />
</div>
```

#### Migration

To keep an icon in the host's color, render it as an `svg` element, or give a `Node::image` the `svg` tag name.

```diff
- Node::image(source)
+ Node::image(source).with_tag_name("svg")
```
