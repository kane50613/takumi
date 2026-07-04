---
packages:
  cargo:takumi-core:
    replay:
      - "exit prerelease: cargo:takumi-core"
---

### Serialize filter, grid track, and gradient values as valid CSS

`filter`/`backdrop-filter` and grid track lists were comma-joined
(`blur(3px), grayscale(0.5)`, `50px, 100px`) where CSS wants spaces, via the
shared `Vec` serializer. `ToCss` now carries a per-type `LIST_SEPARATOR`
(default `, `), overridden to a space for `Filter` and `GridTemplateComponent`.
Linear, radial, and conic gradients also placed the color-interpolation method
after a comma (`to right, in srgb`); it now sits in the leading clause
(`to right in srgb`). The output re-parses instead of dropping these
properties.
