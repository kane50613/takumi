---
packages:
  takumi-core: minor
  takumi-raster: patch
  takumi-svg: patch
  takumi-pdf: patch
---

# An inline element that is positioned or a stacking context paints as a layer of its own

A `display: inline` element with `position`, `opacity`, `filter`, `mix-blend-mode`, `clip-path`, or `mask` now paints as Chrome paints it. Its backgrounds, its text, and the boxes inside it, floats and inline-blocks included, composite as one layer after the content around it. Before, `opacity` faded each line and each text run on its own, and the other properties did nothing. A text node with a style of its own now opens an inline box as a span does. In the paint tree, such an element is a box whose `effects` hold its line boxes and its own text node. `NodePaint::inline_layer`, `InlineLayer`, and `OwnContent::InlineLayer` are new, and `InlineBrush` no longer has an `opacity` field.
