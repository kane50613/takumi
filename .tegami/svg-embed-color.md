---
packages:
  takumi-core:
    type: patch
  takumi-svg:
    type: patch
---

# An SVG image embedded in SVG output takes the host's color whenever it draws with it

SVG output embeds an SVG image with the host's `color` on its root when the image draws with `currentColor`, so a viewer draws it as takumi's own renders do. Takumi used to decide by looking for the text `currentColor` in the markup, so a stylesheet that escaped the keyword drew black in a viewer. It now decides from what the image's layout reads, and leaves an image that never reads the color, or whose root sets its own, as written. `SvgSource::embedded_source` returns that markup. `source_with_current_color` still adds the color whenever the root sets none, and is deprecated.
