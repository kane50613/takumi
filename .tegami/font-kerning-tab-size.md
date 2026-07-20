---
packages:
  "cargo:takumi-core": minor
---

### Add `font-kerning` and `tab-size`

`font-kerning: auto | normal | none` toggles the shaper's `kern` feature; an explicit `font-feature-settings` still wins on a tag conflict. `tab-size: <number>` expands preserved tabs to that many spaces (default 8) before shaping. Preserved tabs previously reached the shaper as U+0009 and rendered a font-dependent glyph, so tab characters under `white-space: pre` now render correctly.
