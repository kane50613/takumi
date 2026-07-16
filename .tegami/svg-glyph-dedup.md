---
packages:
  "cargo:takumi-svg": patch
---

### Deduplicate glyph outlines in SVG output

Glyph outlines land in `<defs>` once and repeat as `<use>` references, shrinking text-heavy documents by 30–75%.
