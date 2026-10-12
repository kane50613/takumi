---
packages:
  "takumi-core": minor
  "takumi": patch
---

# Redraw rounded boxes faster

A small rounded background drawn at the same place and size as in an earlier render reuses its rasterized mask from the glyph cache. `PathCommand::digest` hashes a path for such keys.
