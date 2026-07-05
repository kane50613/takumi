---
packages:
  cargo:takumi-core:
    replay:
      - exit-prerelease(cargo:takumi-core)
---

### Seal `parley` out of the font API

`FontResource::override_info` takes a takumi-owned `FontOverride` (family name,
weight, style, width, axes) instead of `parley`'s `FontInfoOverride`, and
`FontResource::generic_family` takes a takumi-owned `GenericFamily` newtype with
named constants (`GenericFamily::SANS_SERIF`, …) re-exported from the prelude.
`FontSource` is an opaque struct over raw bytes. Callers no longer depend on
`parley`.
