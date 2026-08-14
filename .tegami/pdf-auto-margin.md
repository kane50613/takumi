---
packages:
  takumi-pdf:
    type: minor
---

### Size the page margin to the header and footer

A band draws inside the page margin, so a margin shorter than the band let content run underneath it. Sizing it meant measuring the band first and passing the height back, a second call for something the renderer already knew. `margin` now takes `"auto"` on any side and defaults to it: the side grows to the space its band needs, and never drops below the 48 a page starts with. Left and right hold no band, so they stay at 48.
