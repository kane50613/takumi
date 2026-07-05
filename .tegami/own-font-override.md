---
packages:
  cargo:takumi-core:
    replay:
      - exit-prerelease(cargo:takumi-core)
---

### Seal `parley` out of the font resource API

`FontResource::override_info` now takes a takumi-owned `FontOverride` (owned
family name, weight, style, width, axes) instead of `parley`'s
`FontInfoOverride`. `FontSource` is an opaque struct over raw bytes rather than
an enum exposing a `parley` blob. Callers no longer depend on `parley`.
