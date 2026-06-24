---
packages:
  "npm:@takumi-rs/helpers": patch
---

### Drop the `text` option from `googleFonts`

A `text=` request strips each face's `unicode-range`, so every subset claims full
coverage and overlaps the others, making glyph routing ambiguous and defeating the
render-time codepoint subsetting and the CSS/woff2 caches. Render already downloads
only the glyphs the content uses, so the option was redundant.
