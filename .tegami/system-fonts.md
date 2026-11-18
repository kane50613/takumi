---
packages:
  takumi-core: minor
---

# `Fonts::with_system_fonts` replaces `FontResource::last_resort`

`Fonts::with_system_fonts` creates a `Fonts` with the fonts it is given installed as system fonts, the fonts a renderer starts with. A family name reaches a system font only when no registered font has that name, and font fallback tries the system fonts after every registered family, as `last_resort` fonts did. `unregister_family` never removes a system font.

```rust
let mut fonts = Fonts::with_system_fonts([FontResource::new(geist).override_info(FontOverride {
  family_name: Some("Geist".into()),
  ..Default::default()
})])?;

// A font registered as Geist draws first, and the system Geist draws what it lacks.
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
