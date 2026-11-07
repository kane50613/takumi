---
packages:
  takumi-core: minor
---

# Animations run on pseudo-elements, and registered properties break their cycles

`::before` and `::after` now run their animations, of longhands and custom properties alike. An `animation-*` property that reads an animated custom property keeps the value from before the animation, as Chromium keeps it. A `font-size` that reads a registered length in `em`, or a `line-height` that reads one in `lh`, makes a cycle: the custom property takes its initial value, and the declaration that reads it is dropped, as Chromium breaks it.
