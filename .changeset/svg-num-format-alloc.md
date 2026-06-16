---
"takumi-svg": patch
---

Format coordinates into a stack buffer instead of allocating a String per number (~10% faster glyph-heavy SVG generation)
