---
packages:
  "cargo:takumi-css": patch
---

### Route device-pixel-ratio conversion through one boundary

Device pixels are the canonical unit and `viewport.size` is the source of truth.
`Viewport` and `SizingContext` now expose `to_device`/`to_css` as the only place
the ratio is applied, replacing ad-hoc multiplications across `to_px`, `calc()`,
font-relative collapses, tailwind breakpoints, and image intrinsic sizes.
Rendered output is unchanged.
