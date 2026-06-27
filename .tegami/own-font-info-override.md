---
packages:
  "cargo:takumi-core": minor
  "cargo:takumi-css": minor
---

### Own the font override struct so callers don't depend on fontique

`FontResource::override_info` took a `fontique::FontInfoOverride`. It now takes a
takumi-owned `FontInfoOverride`, re-exported from the prelude, with `FontStretch`,
`FontStyle`, and `FontWeight` fields. `takumi-css` gains `From<parley::FontStyle>
for FontStyle` to support the conversion.
