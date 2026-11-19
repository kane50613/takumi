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

An SVG drawn by `<img>`, `background-image`, or another image now draws `currentColor` in its own `color`, or black when it sets none. Only an `svg` element takes the `color` of the box it sits in, whether it is inline in HTML, a JSX `<svg>`, or a `Node` with the `svg` tag name.

```html
<div style="color: red">
  <!-- Red before and after -->
  <svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="10" fill="currentColor" /></svg>
  <!-- Before: red. After: black -->
  <img src="circle.svg" />
</div>
```

#### Migration

To keep an icon in the host's color, render it as an `svg` element, or give its `Node::image` the `svg` tag name.

```diff
- Node::image(source)
+ Node::image(source).with_tag_name("svg")
```
