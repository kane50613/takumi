---
packages:
  takumi-core: minor
  takumi-raster: patch
  takumi-svg: patch
  takumi-pdf: patch
---

# A positioned or stacking inline element paints as a layer of its own

A `display: inline` element now paints as a layer of its own, as Chrome paints it, when any of these hold:

- `position` is not `static`
- `opacity` is below 1
- `filter` or `backdrop-filter` is not `none`
- `mix-blend-mode` is not `normal`
- `isolation` is `isolate`
- `clip-path` or `mask` is not `none`

Its backgrounds, its text, and the boxes inside it, floats and inline-blocks included, composite together after the content around it. Before, `opacity` faded each line and each text run on its own, and the other properties did nothing.

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

`InlineBrush` no longer has an `opacity` field, since an inline element's `opacity` no longer applies per text run. It applies once, to the layer of the `NodePaint` whose `inline_layer` is set. In the paint tree, that element's box holds the value in `effects.opacity`.

```diff
- let opacity = brush.opacity;
+ let opacity = match &node.kind {
+   NodeKind::Box { effects: Some(effects), .. } => effects.opacity,
+   _ => 1.0,
+ };
```

Here `node` is the element's `PaintNode` in the paint tree.
