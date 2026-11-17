---
packages:
  takumi-core:
    type: minor
  takumi-svg:
    type: patch
---

# An SVG image embedded in SVG output takes the host's color whenever it draws with it

In SVG output, an embedded SVG image that draws with `currentColor` now gets the host's `color` on its root, so a viewer draws it as takumi's own renders do. Before, the color was added only when the markup spelled out `currentColor`, so a stylesheet that escaped the keyword drew black in a viewer. An image that does not read the color at the size it is drawn, or whose root sets its own `color`, stays as written.

#### Migration

`SvgSource::source_with_current_color` is removed. `SvgSource::embedded_source` adds the color only when the image reads it.

```diff
- let markup = source.source_with_current_color(color);
+ let markup = source.embedded_source(&host, viewport);
```
