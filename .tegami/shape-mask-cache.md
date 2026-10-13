---
packages:
  "takumi-core": minor
  "takumi": patch
---

# Redraw rounded boxes faster

A rounded background drawn at the same place and size as in an earlier render reuses its rasterized mask from the glyph cache. A mask may take at most `glyph_cache::max_entry_bytes`, a 32nd of the budget. `PathCommand::digest` hashes a path for such keys.
