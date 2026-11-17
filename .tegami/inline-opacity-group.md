---
packages:
  takumi-core: minor
  takumi-raster: patch
  takumi-svg: patch
  takumi-pdf: patch
---

# A positioned or stacking inline element paints as a layer of its own

A `display: inline` element with `position`, `opacity`, `filter`, `mix-blend-mode`, `clip-path`, or `mask` now paints as a layer of its own, as Chrome paints it. Its backgrounds, its text, and the boxes inside it, floats and inline-blocks included, composite together after the content around it. Before, `opacity` faded each line and each text run on its own, and the other properties did nothing.

```html
<!-- Before: each line and each text run fades on its own -->
<!-- After: the whole element fades as one layer -->
<span style="opacity: 0.5; background: gold">A span that wraps onto a second line</span>
```

- The layer's box is the union of the element's line boxes. This approximates Chrome, where each line's box stays apart.
- A text node with a style of its own now opens an inline box as a span does.
- In the paint tree, such an element is a box with `effects`, and its line boxes and its own text node are its children.
- `NodePaint::inline_layer`, `InlineLayer`, and `OwnContent::InlineLayer` are new.

#### Migration

`InlineBrush` no longer has an `opacity` field. An inline element's `opacity` now applies once, as an effect in the `properties` of the `NodePaint` whose `inline_layer` is set.

```diff
- let opacity = brush.opacity;
+ // Read the opacity from the effects in `node_paint.properties`
```
