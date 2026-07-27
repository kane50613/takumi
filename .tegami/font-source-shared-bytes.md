---
packages:
  "cargo:takumi-core": minor
---

### Register a font from bytes the caller already holds

`FontSource::from_shared` takes an `Arc<dyn AsRef<[u8]> + Send + Sync>` and passes it to the font system untouched, so a memory-mapped face stays paged from disk instead of being copied onto the heap, which for a CJK family is tens of megabytes that never reach the heap at all. WOFF and WOFF2 still decompress into a fresh buffer.
