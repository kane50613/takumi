---
"takumi-pdf": minor
---

# Render through characters no font covers with `missingGlyph`

`missingGlyph: "notdef"` draws the font's `.notdef` glyph and `"skip"` drops the character, instead of the default `"error"` that fails the render.
