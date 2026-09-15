---
packages:
  "takumi": minor
---

# Walk each glyph run once and remember what repeats

Resolving a run's glyphs walked parley's glyph iterator twice; it now collects the positioned glyphs once and resolves from that list, and the glyph resolver is built only on the first cache miss. `text-decoration-skip-ink` remembers the intercepts of an outline at a given band, and `line-height: normal` is resolved once per render for each family, attribute and size combination. Output is unchanged; the 24-card paint tree builds in 7.1 ms instead of 9.5 ms.
