---
packages:
  "takumi-core": minor
---

# Key resolved glyphs with a faster hasher

`PositionedInlineRun::resolved_glyphs` is now a `ResolvedGlyphs`, a `FxHashMap` keyed by glyph id, instead of a `HashMap` with the default hasher. Code that only calls `get` on it keeps working.
