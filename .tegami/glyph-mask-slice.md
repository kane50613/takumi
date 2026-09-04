---
packages:
  takumi-core:
    type: minor
---

### Hand out a glyph mask as a slice

`glyph_cache::glyph_mask` returns `Arc<[u8]>` in place of `Arc<Vec<u8>>`.
