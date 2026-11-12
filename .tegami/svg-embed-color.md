---
packages:
  takumi-core:
    type: patch
  takumi-svg:
    type: patch
---

# An SVG image embedded in SVG output always draws in the host's color

SVG output embeds an SVG image with the host's `color` on its root, so its `currentColor` draws as takumi's own renders draw it. The color used to be added only when the markup spelled `currentColor` out, so a stylesheet that escaped the keyword drew black in a viewer. A root with a `color` of its own keeps it.
