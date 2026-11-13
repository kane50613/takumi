---
packages:
  takumi-core: minor
  takumi-raster: patch
  takumi-svg: patch
  takumi-pdf: patch
---

# An inline element that is positioned or a stacking context paints as a layer of its own

A `display: inline` element with `position`, `opacity`, `filter`, `mix-blend-mode`, `clip-path`, or `mask` now paints as a layer of its own, as Chrome paints it. Its backgrounds, its text, and the boxes inside it, floats and inline-blocks included, composite together after the content around it. The layer's box is the union of the element's line boxes, which approximates Chrome, where each line's box stays apart. Before, `opacity` faded each line and each text run on its own, and the other properties did nothing. A text node with a style of its own now opens an inline box as a span does. In the paint tree, such an element is a box with `effects`, and its line boxes and its own text node are its children. `NodePaint::inline_layer`, `InlineLayer`, and `OwnContent::InlineLayer` are new, and `InlineBrush` no longer has an `opacity` field.
