---
packages:
  "cargo:takumi-core": minor
---

### Register a font straight out of the binary

`FontSource::from_static` takes a `&'static [u8]`, so an `include_bytes!` face goes to the font system where it already sits, in the read-only segment, with no `Arc` to build around it. Its blob id comes from the address and length instead of a hash of the content, so registering a 30 MiB CJK face no longer reads through every page of it and the face is paged in a glyph at a time.
