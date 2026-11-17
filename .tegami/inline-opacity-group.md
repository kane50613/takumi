---
packages:
  takumi-core: minor
  takumi-raster: patch
  takumi-svg: patch
  takumi-pdf: patch
---

# A positioned or stacking inline element paints as a layer of its own

A `display: inline` element now paints as a layer of its own, as Chrome paints it, when it sets any of these:

- `position`
- `opacity`
- `filter`
- `mix-blend-mode`
- `clip-path`
- `mask`

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
- brush.opacity          // on each text run of the element
+ box.effects.opacity    // once, on the element's box in the paint tree
```
