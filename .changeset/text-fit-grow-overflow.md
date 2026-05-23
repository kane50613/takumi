---
"takumi": patch
---

Fix `text-fit: grow` causing text to disappear (or panic) on elements without an explicit width when combined with `background-clip: text`. During intrinsic measure, the unconstrained-width sentinel was finite (`f32::MAX`), so `text_fit_line_scales`'s `is_finite()` guard missed it and computed a near-infinite scale, producing a `layout.size.height ≈ f32::MAX` that overflowed `rasterize_layers`. Switch the sentinel to `f32::INFINITY` so existing `is_finite()` guards skip text-fit during max-content measurement, matching Chrome's behavior where text-fit only applies under a definite containing block.
