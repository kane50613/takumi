---
packages:
  "npm:@takumi-rs/helpers": patch
---

### Rename the `googleFonts` family field from `family` to `name`

A `GoogleFontFamily` object now spells its family as `name`, not `family` —
`googleFonts({ families: [{ name: "Inter", weight: [400, 700] }] })`. Reads
cleaner next to `families` and matches the `name` field on rendered fonts. Bare
string families are unchanged.
