---
packages:
  "cargo:takumi-core": minor
  "cargo:takumi-raster": patch
---

### Share the glyph caches across worker threads

The glyph mask and resolved-glyph caches were thread-local maps under one process-wide byte counter, but eviction only pruned the inserting thread — an idle worker kept its share forever, so real retention multiplied with the thread pool (the emoji-bitmap growth noted in #1023). Both caches are now process-global `quick_cache` instances splitting the same 8 MiB budget: a glyph resolved on one thread is a hit on every thread, and eviction is global. `GlyphCache` methods take `&self` and `get` returns a clone; `set_glyph_cache_max_bytes` now applies to caches not yet used, so call it before the first render.
