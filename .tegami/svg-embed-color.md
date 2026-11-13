---
packages:
  takumi-core:
    type: patch
  takumi-svg:
    type: patch
---

# An SVG image embedded in SVG output takes the host's color whenever it draws with it

SVG output now adds the host's `color` to an embedded SVG image's root exactly when the image's layout, at the size it is drawn, reads it, including through an escaped `currentColor`. `SvgSource::embedded_source` returns that markup, and `source_with_current_color` is deprecated.
