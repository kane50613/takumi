---
packages:
  takumi-core:
    type: minor
  takumi-pdf:
    type: minor
---

### Draw dashed, dotted and double borders in PDF

`border-style: dashed`, `dotted` and `double` came out solid. PDF now strokes the border's centerline with the dash pattern the other backends already used, and fills two rings for a double border. An `outline` with those styles follows, since it paints through the same code.

`border_paint` picks between the two, so a backend no longer decides for itself whether a border strokes or fills.
