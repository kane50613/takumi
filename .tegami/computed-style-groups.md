---
packages:
  "takumi-core": minor
  "takumi": patch
  "takumi-pdf": patch
  "takumi-svg": patch
  "takumi-raster": patch
---

# Share computed styles between boxes

Boxes share the parts of their computed style they leave unchanged, as Chrome's do. An 11,000-row table takes about a quarter less memory.

`ComputedStyle` fields now sit in groups named after Blink's, each behind an `Rc`. `RenderContext::style` is an `Rc<ComputedStyle>`, and `RenderContextBuilder::style` still takes a `Box<ComputedStyle>`.

```rust
let width = style.box_data.width;
let color = style.inherited_data.color;

Rc::make_mut(&mut style.box_data).width = Size::default();
```
