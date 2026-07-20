---
packages:
  "cargo:takumi-core": minor
---

### Move resolved-glyph types to `resources::glyph`

Glyph rasterization — resolving a shaped glyph to a bitmap or vector outline — now lives in its own `resources::glyph` module, split out of the font registry it was tangled with. `ResolvedGlyph`, `ResolvedOutlineGlyph`, and `ResolvedColorLayer` move there from `resources::font`; imports of those types need updating.
