---
packages:
  cargo:takumi-core:
    type: minor
  cargo:takumi:
    type: major
---

### Keep taffy, parley, and tiny_skia out of the public API

The layout and paint engine now lives behind a new `unstable` feature. `layout::{tree, inline, border}`, `paint`, `scene`, `geometry`, and `shadow` — which trade in `taffy`, `parley`, and `tiny_skia` types — are only compiled with that feature; the backend crates enable it. The default `takumi-core` API no longer exposes a foreign type, so a consumer can parse and hold styles without depending on the layout engine's crates.

`FontWeight::Absolute` now holds an `f32` instead of a `parley` weight. `BackgroundSize::resolve` takes a `(u32, u32)` area rather than a `taffy::Size`. `BorderProperties`'s `width`/`color`/`style` fields are `Sides<T>` instead of `taffy::Rect<T>`. The `ours -> taffy`/`ours -> parley` `From` impls are gone; conversions moved to internal methods or the backend crates. The mistyped `FontSynthesic` enum is now `FontSynthesisMode`. `StyleSheetParseError`, `StyleSheetParseErrorKind`, `StyleDeclarationBlockParseError`, `RegisteredFamily`, and `RegisteredFace` are `#[non_exhaustive]`.
