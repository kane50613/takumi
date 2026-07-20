---
packages:
  "cargo:takumi-core": minor
  "cargo:takumi-raster": patch
---

### Bound the thread-local glyph caches by bytes

The resolved-glyph and glyph-mask caches were thread-local maps capped at 4096 entries each: per-entry size was unbounded, the cap multiplied per worker thread, and overflowing flushed the whole map so hot glyphs paid the rebuild cost. Both caches now weigh entries in bytes against one process-wide 8 MiB budget, tunable through `takumi_core::resources::glyph_cache::set_glyph_cache_max_bytes`. Going over budget first drops entries no recent render touched, then only as many fresh ones as the overage requires; the map is never flushed whole. A retention test renders the same content 200 times and asserts live heap bytes stay flat, so budget regressions fail in CI.
