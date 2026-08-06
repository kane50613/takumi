---
packages:
  takumi-pdf:
    type: minor
---

### Apply the color `filter` primitives

`grayscale`, `sepia`, `saturate`, `hue-rotate`, `invert`, `brightness`, `contrast` and `opacity` now apply. They are linear transforms of the source color, so they fold into the colors written to the page, including gradient stops, text and decoded image pixels, instead of rasterizing the element.

- filters apply in order, clamping between them as CSS requires, and compose down the stacking contexts so a filtered ancestor reaches its descendants
- SVG images rasterize while a filter is active, since the transform applies to pixels
- shadows follow the filter too, like every other color the element paints
- `blur()` and `drop-shadow()` need a convolution and are still ignored, as are referenced SVG filters
- transforming each color before compositing matches compositing first only while the filtered content is opaque; overlapping translucent content differs
