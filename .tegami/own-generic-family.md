---
packages:
  "cargo:takumi-core": minor
---

### Own `GenericFamily` so callers don't depend on `parley`

`FontResource::generic_family` took a `parley::GenericFamily`, forcing callers
to add `parley` as a dependency. It now takes a takumi-owned `GenericFamily`
newtype exposing the families as named constants (`GenericFamily::SANS_SERIF`,
etc.), re-exported from the prelude.
