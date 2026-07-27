---
packages:
  "cargo:takumi-core": minor
---

### Register a font straight out of the binary

`FontSource::from_static` takes a `&'static [u8]`, so an `include_bytes!` face is read where it already sits, in the read-only segment, and the caller writes no `Arc` of its own: the one held internally wraps the slice reference, never a copy of the font. Its blob id comes from the address and length instead of a hash of the content, so registering a 30 MiB CJK face no longer reads through every page of it and the face is paged in a glyph at a time.
