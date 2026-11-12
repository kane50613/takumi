---
packages:
  takumi-core: minor
---

# A translucent inline element fades as one group

An inline element's `opacity` now fades its background, border, and text together with everything inside it, as Chrome paints it. Before, the text and background of an element nested in a translucent span stayed opaque, and where two lines of the span overlapped, they showed darker. In the paint tree, the element gets a box whose `effects` carry the opacity and hold its line boxes and its own text node. The line box of a span nested in another now sits inside the outer span's line box. `InlineBrush` no longer has an `opacity` field.
