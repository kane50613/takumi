---
packages:
  takumi-core: minor
---

# `Fonts::with_system_fonts` replaces `FontResource::last_resort`

`Fonts::with_system_fonts` creates a `Fonts` with the fonts it is given installed as system fonts, the fonts a renderer starts with. A family name reaches a system font only when no registered font has that name. `unregister_family` never removes a system font.

Font fallback tries the system fonts last, as it did `last_resort` fonts. By default they come after every registered family. A snapshot from `snapshot_with_fallbacks` tries only the families it lists before the system fonts, and a system family it lists by name draws at its place in that list.

```rust
let mut fonts = Fonts::with_system_fonts([FontResource::new(geist).override_info(FontOverride {
  family_name: Some("Geist".into()),
  ..Default::default()
})])?;

// A font registered as Geist draws first. The system Geist draws a character it lacks
// when no other registered font has that character.
fonts.register(FontResource::new(geist_cyrillic).override_info(FontOverride {
  family_name: Some("Geist".into()),
  ..Default::default()
}))?;
```

A system font joins no generic family, so `generic_family` on a font passed to `with_system_fonts` does nothing.

#### Migration

`FontResource::last_resort` is removed. Pass those fonts to `Fonts::with_system_fonts` when you create the `Fonts`, instead of registering them.

```diff
- let mut fonts = Fonts::default();
- fonts.register(FontResource::new(geist).last_resort())?;
+ let mut fonts = Fonts::with_system_fonts([FontResource::new(geist)])?;
```
