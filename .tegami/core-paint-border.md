---
packages:
  takumi-core:
    type: minor
---

### Paint a border ring from one place

`paint_border` fills a ring, fills two for a `double` border, or strokes the centerline for a uniform `dashed` or `dotted` one, driving whichever backend supplied the device. It returns whether it painted, so a backend only walks the sides itself when the border needs per-side work.

`PaintDevice` gains `stroke_shape`, with a default that does nothing for a backend without a stroker.

A fully transparent border no longer reaches the output.
