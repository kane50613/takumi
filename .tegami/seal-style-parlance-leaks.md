---
packages:
  cargo:takumi-core: minor
---

### Seal the `parlance` font model out of the public `style` API

`#916` grepped only `parley::`, so it missed `parlance` (the parley font
model) leaking through `style`. This follow-up seals it:

`FontFeature`, `FontVariation`, and `Tag` are now takumi-owned structs,
replacing `parlance::tag::{FontFeature, FontVariation, Tag}` in
`ComputedStyle::font_feature_settings`/`font_variation_settings`.
`FontWeight::Absolute` holds a plain `f32` instead of
`parlance::font::FontWeight`. `ComputedStyle::lang` is now `Option<Lang>`, a
takumi-owned BCP-47 tag, instead of `Option<parlance::language::Language>`.
`FontStretch`, `FontStyle`, `FontWeight`, `FontFamily`, and
`resources::font::GenericFamily` lose their `From<_>`/`Into<_>` impls
targeting `parlance` types; the conversions are now `pub(crate)` inherent
methods (`into_parlance`/`to_parlance`/`from_parlance_generic`) called only
at the shaping boundary.

Also dropped two unused `From<FontFamily> for parlance::FontFamily` impls
with no callers in the crate.
