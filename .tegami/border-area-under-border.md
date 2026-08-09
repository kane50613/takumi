---
packages:
  takumi-svg:
    type: patch
  takumi-raster:
    type: patch
---

### Paint a `border-area` background under the border

`background-clip: border-area` fills the ring the border strokes. All three backends put that fill in the wrong place: the SVG and PDF backends emitted it after the border, and the raster backend used it as the border's paint source instead of the border's own colour. Either way the border was lost.

`background-clip` only picks the shape a background fills, never when it paints. The fill now runs in the background phase like every other clip, and the border paints over it.
