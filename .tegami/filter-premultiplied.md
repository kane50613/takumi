---
packages:
  "@takumi-rs/core":
    type: patch
---

### Read premultiplied pixels as straight colour in `filter`

A colour filter ran on the canvas's premultiplied pixels as if they held straight colour, so it only landed on the right value where the element was fully opaque. Antialiased edges came out washed out, and `opacity()` left the colour unscaled, so a faded layer went pale instead of translucent.
