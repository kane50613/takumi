---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Paint backgrounds as Chrome does

- `background-repeat: space` puts the first and last tiles on the edges. `round` rounds to the nearest tile count.
- Shorter `background-size`, `-position`, `-repeat` and `-blend-mode` lists cycle over the layers.
- `background-blend-mode` in image output blends only with the box's own layers, not with what is behind it.
- Gradient hard stops land on Chrome's pixels.
- A repeating gradient with all stops at one position paints its last color.
- A tile smaller than 1/64px paints nothing.
- SVG places tiles at exact positions. PDF no longer repeats a layer along an axis that does not repeat.
- A stray token, as in `background-repeat: repeat bogus`, makes the declaration invalid.
