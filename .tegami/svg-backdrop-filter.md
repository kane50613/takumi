---
packages:
  cargo:takumi-svg:
    replay:
      - "exit prerelease: cargo:takumi-svg"
  cargo:takumi-core:
    replay:
      - "exit prerelease: cargo:takumi-core"
---

### Render backdrop-filter in the SVG backend

SVG has no native backdrop source, so the backdrop is the scene replayed up to
the element, run through an SVG `<filter>` chain, then clipped to the border
box and attenuated by the element's mask and clip-path. Adds
`ComputedStyle::has_shape_mask` and `Filter::is_drop_shadow`.
