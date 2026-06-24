---
packages:
  "npm:@takumi-rs/helpers": minor
---

### Subset Google Fonts inside `render`

`googleFonts({ families })` returns every coverage subset of each family, with its
`unicode-range`, a distinct name under one `subsetOf`, and a stable key. `render`
registers only the subsets the content draws, so a multilingual image pulls a
handful of blocks instead of whole fonts. Set `subset: false` to register
everything; call `subsetFonts({ fonts, source })` to trim a set yourself.

This replaces `googleFont` and `googleFontSubsets` with one object-shaped
`googleFonts`. Distinct subset names mean a glyph routes to the file that covers it
rather than a same-named sibling that lacks it.
