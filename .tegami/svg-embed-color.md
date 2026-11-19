---
packages:
  takumi-core:
    type: minor
  takumi-svg:
    type: patch
---

# An `svg` element embedded in SVG output takes the host's color whenever it draws with it

In SVG output, an embedded `svg` element that draws with `currentColor` now carries the host's `color` on its root, so a viewer draws it as takumi does. Before, it got the color only when its markup spelled out `currentColor`.

#### Migration

`SvgSource::source_with_current_color` is removed.

```diff
- let markup = source.source_with_current_color(color);
+ let markup = source.embedded_source(&host, viewport);
```
