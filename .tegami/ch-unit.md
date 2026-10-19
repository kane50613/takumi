---
packages:
  takumi-core: minor
---

# Support the `ch` unit

`ch` lengths, alone or in `calc()`, resolve to the advance of the `0` glyph in the first font that covers it, or half the font size when that font has no `0`.
