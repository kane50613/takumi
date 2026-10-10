---
packages:
  "takumi-core": minor
  "takumi": patch
  "takumi-pdf": patch
  "takumi-svg": patch
  "takumi-raster": patch
---

# Keep a computed style in Blink's field groups

Large documents render in less memory. Boxes share the parts of their computed style they leave unchanged, as Chrome's do. An 11,000-row table takes about a quarter less memory.

`ComputedStyle` fields now sit in groups named after Blink's field groups, each behind an `Rc`:

```rust
let width = style.box_data.width;
let color = style.inherited_data.color;

Rc::make_mut(&mut style.box_data).width = Size::default();
```
