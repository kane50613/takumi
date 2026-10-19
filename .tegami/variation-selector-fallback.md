---
packages:
  takumi-core: patch
---

# Honor emoji and text variation selectors in font fallback

Since 2.17.0, the first font with a glyph drew a character whatever variation selector followed it, so `‼️` came out monochrome after a text font and `‼︎` in color after an emoji font. A font that lacks a variation sequence now hands it to a later font that draws it the way it asks, as Chrome does, and only when none does does the base character's glyph stand in.
