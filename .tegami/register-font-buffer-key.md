---
packages:
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
  "@takumi-rs/helpers": patch
---

# `registerFont` registers one buffer again under another name, weight, style or `unicodeRange`

Passing the same `data` buffer to `registerFont` a second time with other descriptors used to return the first registration, so the new family or weight never existed. Each set of descriptors now registers its own face, as `@font-face` rules sharing one `src` do. A lazy font's key includes its `unicodeRange` too.
