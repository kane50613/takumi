---
packages:
  takumi-core: minor
  takumi-raster: patch
---

# Strokes and dashes are outlined closer to Chromium

Strokes, dashed and dotted borders, and SVG strokes now follow Chromium's outlines more closely.

- A curve that doubles back on itself gets a rounded point where it turns.
- A dashed line of no length draws its round or square cap.
- The edges of round caps and joins can shift by a few colour levels.

`takumi_core::path` gains `Path::stroke`, `Path::dash`, `Stroke`, and `StrokeDash`, which outline a path for any backend to fill.

```rust
use takumi_core::{
  path::{PathBuilder, Stroke, StrokeDash},
  style::LineCap,
};

let mut builder = PathBuilder::new();

builder.move_to(0.0, 10.0);
builder.line_to(100.0, 10.0);

let stroke = Stroke {
  width: 4.0,
  line_cap: LineCap::Round,
  ..Stroke::default()
};
let outline = builder
  .finish()
  .zip(StrokeDash::new(vec![8.0, 4.0], 0.0))
  .and_then(|(path, dash)| path.dash(&dash, 1.0))
  .and_then(|dashed| dashed.stroke(&stroke, 1.0));
```
