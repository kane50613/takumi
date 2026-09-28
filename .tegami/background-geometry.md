---
packages:
  "takumi": patch
  "takumi-pdf": patch
  "takumi-paint": patch
---

# Place background tiles where Chrome places them

- Background and mask tiles are sized, positioned and snapped to pixels in 1/64px layout units, as Chrome does. `round`, `space`, `cover` and `contain` tiles now land on the same pixels as Chrome's.
- A tile that sits outside the box, or under an opaque border, no longer paints there.
- A repeating layer seen through `background-clip: text` in PDF repeats across the whole text instead of showing one tile.
- An image or background drawn through a rounded or clipped shape no longer shifts half a pixel and blurs in the image output.
- A `pattern` paint carries the `area` its tiles show in.
