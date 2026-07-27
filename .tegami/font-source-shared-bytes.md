---
packages:
  "cargo:takumi-core": minor
---

### Register a font from bytes the caller already holds

`FontSource::from_shared` takes an `Arc<dyn AsRef<[u8]> + Send + Sync>` and passes it to the font system untouched, so a memory-mapped face stays paged from disk instead of being copied onto the heap. For a CJK family that is tens of megabytes that never reach the heap at all. woff and woff2 still decompress into a fresh buffer.
