## takumi-paint@0.1.1

### Write glyph outlines only when a run asks for them

`paint()` no longer writes every run's outline up front. `run.outline()` builds it when called. A 2,000-section document paints 3 times faster in about a third of the memory.

In Rust, `TextRun::outline` is replaced by `PaintTree::run_outline(node, run)`.

### Keep less memory for each painted tree

A painted tree no longer keeps a second copy of itself in WebAssembly memory. Keeping several large trees at once takes about a third less memory.

## takumi-paint@0.1.0

### Get what to draw from JSX, HTML, or a node tree

`paint()` lays out the input and returns a `PaintTree` instead of pixels. `nodes` lists each box, text run and image with its CSS resolved. `steps` lists what to draw, in paint order.
