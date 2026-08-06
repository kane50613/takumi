---
packages:
  takumi-pdf:
    type: patch
---

### Write less per embedded font

`/DW` now states the width most glyphs share, so `/W` only lists the ones that differ, which empties it almost entirely for monospaced and CJK faces. The `CIDSet` stream is gone: only PDF/A-1b asks for one, and that level is not offered. `/FontBBox` comes from the subset's own box instead of a pass over every glyph outline.
