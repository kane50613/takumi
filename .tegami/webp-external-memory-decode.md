---
packages:
  "cargo:takumi-core": patch
---

### Halve peak memory for native WebP decode

Native WebP decode now writes straight into a caller-owned buffer via `WebPDecode` with external memory, dropping the extra full-frame copy `WebPDecodeRGBA` required. Already-RGBA sources decoded through the `image` crate also skip one transient full-frame clone (`into_rgba8`). Output is bit-identical.
